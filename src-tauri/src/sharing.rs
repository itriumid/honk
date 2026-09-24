//! `.honk` files: a whole library, or one category, packed into a zip to share.
//!
//! A `.honk` holds `manifest.json` and `sounds/<sha256>.<extension>` for each sound. The manifest
//! lists the categories and the sounds in pad order, with each sound's name, volume, favorite
//! flag, hotkey and category.
//!
//! Files come from other people, so reading one treats it as hostile. Paths inside the archive
//! are never used to write anything; each sound is found by the name the manifest implies. Every
//! sound is size-capped, has to match its hash, and has to decode, and all of it is checked
//! before anything is added. The hash catches a damaged file, not a tampered one: whoever edits a
//! file can recompute it, so safety comes from those checks, not from trusting the author.
//!
//! Files aren't encrypted. They exist to be shared, a password would have to travel with them,
//! and what they need protecting from is the reader, not the other way around.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::hotkeys;
use crate::library::{Library, NewSound, Sound};

const FORMAT: &str = "honk";
/// Bumped only for a change older versions of Honk can't read; they refuse newer files.
const VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";

const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SOUND_BYTES: u64 = 100 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_SOUNDS: usize = 10_000;
const MAX_NAME_CHARACTERS: usize = 200;

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    /// The Honk version that wrote the file, for messages; nothing depends on it.
    app_version: String,
    categories: Vec<String>,
    sounds: Vec<ManifestSound>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestSound {
    name: String,
    sha256: String,
    extension: String,
    volume: f32,
    favorite: bool,
    hotkey: Option<String>,
    category: Option<String>,
}

/// Read first and leniently, so a file from a newer Honk gets a clear message rather than
/// whatever the stricter parse trips over.
#[derive(Deserialize)]
struct Header {
    format: Option<String>,
    version: Option<u32>,
    app_version: Option<String>,
}

impl ManifestSound {
    fn entry_name(&self) -> String {
        format!("sounds/{}.{}", self.sha256, self.extension)
    }
}

#[derive(Debug, Serialize)]
pub struct ExportSummary {
    pub sounds: usize,
    pub categories: usize,
}

/// Writes the library, or only `category`, to `destination`. The file appears whole or not at
/// all: it's written alongside first, then moved into place.
pub fn export(
    library: &Library,
    category: Option<i64>,
    destination: &Path,
) -> Result<ExportSummary, String> {
    let all_categories = library.categories()?;
    let categories: Vec<_> = match category {
        Some(id) => all_categories
            .iter()
            .filter(|candidate| candidate.id == id)
            .collect(),
        None => all_categories.iter().collect(),
    };
    if category.is_some() && categories.is_empty() {
        return Err("that category no longer exists".to_string());
    }
    let category_name = |id: Option<i64>| {
        id.and_then(|id| all_categories.iter().find(|candidate| candidate.id == id))
            .map(|found| found.name.clone())
    };
    let sounds: Vec<Sound> = library
        .list()?
        .into_iter()
        .filter(|sound| category.is_none() || sound.category_id == category)
        .collect();

    let partial = partial_path(destination);
    let written = write_archive(library, &sounds, &categories, &category_name, &partial);
    if let Err(error) = written {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    fs::rename(&partial, destination).map_err(|error| {
        let _ = fs::remove_file(&partial);
        format!("could not save the file: {error}")
    })?;
    Ok(ExportSummary {
        sounds: sounds.len(),
        categories: categories.len(),
    })
}

fn write_archive(
    library: &Library,
    sounds: &[Sound],
    categories: &[&crate::library::Category],
    category_name: &dyn Fn(Option<i64>) -> Option<String>,
    path: &Path,
) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("could not save the file: {error}"))?;
    let mut archive = ZipWriter::new(BufWriter::new(file));
    let mut entries = Vec::with_capacity(sounds.len());

    for sound in sounds {
        let stored = library.stored(sound.id)?;
        let entry = ManifestSound {
            name: sound.name.clone(),
            sha256: stored.content_hash,
            extension: stored.extension,
            volume: sound.volume,
            favorite: sound.favorite,
            hotkey: sound.hotkey.clone(),
            category: category_name(sound.category_id),
        };
        // Compressed audio doesn't shrink any further; only uncompressed WAV is worth deflating.
        let method = if entry.extension == "wav" {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        let options = SimpleFileOptions::default()
            .compression_method(method)
            .large_file(stored.bytes.len() as u64 >= u32::MAX as u64);
        archive
            .start_file(entry.entry_name(), options)
            .and_then(|()| archive.write_all(&stored.bytes).map_err(Into::into))
            .map_err(archive_error)?;
        entries.push(entry);
    }

    let manifest = Manifest {
        format: FORMAT.to_string(),
        version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        categories: categories
            .iter()
            .map(|category| category.name.clone())
            .collect(),
        sounds: entries,
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    archive
        .start_file(
            MANIFEST,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .and_then(|()| archive.write_all(&json).map_err(Into::into))
        .map_err(archive_error)?;
    archive
        .finish()
        .and_then(|mut writer| writer.flush().map_err(Into::into))
        .map_err(archive_error)?;
    Ok(())
}

/// What importing a file would do, shown before anything is written.
#[derive(Debug, Serialize)]
pub struct Preview {
    /// The Honk version that wrote the file.
    pub made_with: String,
    pub sounds: usize,
    pub new_sounds: usize,
    /// Sounds whose audio is already in the library; they're left exactly as they are.
    pub duplicates: usize,
    /// New sounds the file puts in no category; they stay in none, whatever the window shows.
    pub uncategorized: usize,
    pub categories: Vec<PreviewCategory>,
    /// Hotkeys the new sounds would bring, if hotkeys are imported.
    pub hotkeys: Vec<PreviewHotkey>,
}

#[derive(Debug, Serialize)]
pub struct PreviewCategory {
    pub name: String,
    /// A category with this name, ignoring case, already exists, and the sounds join it.
    pub merges: bool,
    /// New sounds going into it.
    pub sounds: usize,
}

#[derive(Debug, Serialize)]
pub struct PreviewHotkey {
    pub hotkey: String,
    pub sound: String,
    /// What already has this hotkey, named for display. Only these can be replaced.
    pub taken_by: Option<String>,
    /// Another sound earlier in the same file has it, so this one never gets it.
    pub repeated: bool,
}

pub fn preview(library: &Library, path: &Path) -> Result<Preview, String> {
    let mut archive = open(path)?;
    let manifest = read_manifest(&mut archive)?;
    let existing: HashSet<String> = library
        .categories()?
        .into_iter()
        .map(|category| category.name.to_lowercase())
        .collect();

    let mut new_sounds = 0;
    let mut duplicates = 0;
    let mut uncategorized = 0;
    let mut per_category = std::collections::HashMap::<String, usize>::new();
    let mut seen_hashes = HashSet::new();
    let mut seen_hotkeys = HashSet::new();
    let mut hotkeys = Vec::new();
    for_each_sound(&mut archive, &manifest, |sound, _| {
        if !seen_hashes.insert(sound.sha256.clone())
            || library.find_by_content_hash(&sound.sha256)?.is_some()
        {
            duplicates += 1;
            return Ok(());
        }
        new_sounds += 1;
        match &sound.category {
            Some(name) => *per_category.entry(name.to_lowercase()).or_default() += 1,
            None => uncategorized += 1,
        }
        if let Some(hotkey) = &sound.hotkey {
            let repeated = !seen_hotkeys.insert(hotkey.clone());
            hotkeys.push(PreviewHotkey {
                hotkey: hotkey.clone(),
                sound: sound.name.clone(),
                taken_by: library
                    .hotkey_holder(hotkey)?
                    .map(|holder| holder.describe()),
                repeated,
            });
        }
        Ok(())
    })?;

    Ok(Preview {
        made_with: manifest.app_version.clone(),
        sounds: manifest.sounds.len(),
        new_sounds,
        duplicates,
        uncategorized,
        categories: manifest
            .categories
            .iter()
            .map(|name| PreviewCategory {
                name: name.clone(),
                merges: existing.contains(&name.to_lowercase()),
                sounds: per_category.get(&name.to_lowercase()).copied().unwrap_or(0),
            })
            .collect(),
        hotkeys,
    })
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub added: usize,
    /// The one category every added sound went into, for the window to switch to. `None` when
    /// they're spread across several, or some are in none: the window shows everything then.
    pub lands_in: Option<i64>,
    pub duplicates: usize,
    pub hotkeys_assigned: usize,
    pub hotkeys_skipped: usize,
}

/// Imports the file. Hotkeys come along only with `import_hotkeys`; one that something else
/// already has is taken from it only when it's in `replace`, and skipped otherwise.
pub fn import(
    library: &Library,
    path: &Path,
    import_hotkeys: bool,
    replace: &HashSet<String>,
) -> Result<ImportReport, String> {
    let mut archive = open(path)?;
    let manifest = read_manifest(&mut archive)?;
    // The whole file is checked before anything is added, so a bad sound halfway through
    // doesn't leave half an import behind. The adding pass checks each sound again as it
    // reads it, in case the file changed in between.
    for_each_sound(&mut archive, &manifest, |_, _| Ok(()))?;

    let mut category_ids = std::collections::HashMap::new();
    for name in &manifest.categories {
        category_ids.insert(name.to_lowercase(), library.category_named(name)?.id);
    }

    let mut report = ImportReport::default();
    let mut added_hotkeys = Vec::new();
    let mut landed = HashSet::new();
    for_each_sound(&mut archive, &manifest, |sound, bytes| {
        let category = sound
            .category
            .as_ref()
            .and_then(|name| category_ids.get(&name.to_lowercase()).copied());
        let (added, duplicate) = library.add(NewSound {
            name: &sound.name,
            bytes,
            extension: &sound.extension,
            volume: sound.volume,
            favorite: sound.favorite,
            category,
        })?;
        if duplicate {
            report.duplicates += 1;
        } else {
            report.added += 1;
            landed.insert(added.category_id);
            if let Some(hotkey) = &sound.hotkey {
                added_hotkeys.push((added.id, hotkey.clone()));
            }
        }
        Ok(())
    })?;

    report.lands_in = match landed.into_iter().collect::<Vec<_>>().as_slice() {
        [Some(id)] => Some(*id),
        _ => None,
    };

    if import_hotkeys {
        let mut assigned = HashSet::new();
        for (id, hotkey) in added_hotkeys {
            // A hotkey repeated within the file goes to the first sound that has it.
            if assigned.contains(&hotkey) {
                report.hotkeys_skipped += 1;
                continue;
            }
            if library.hotkey_holder(&hotkey)?.is_some() {
                if !replace.contains(&hotkey) {
                    report.hotkeys_skipped += 1;
                    continue;
                }
                library.release_hotkey(&hotkey)?;
            }
            library.set_hotkey(id, Some(&hotkey))?;
            assigned.insert(hotkey);
            report.hotkeys_assigned += 1;
        }
    } else {
        report.hotkeys_skipped = added_hotkeys.len();
    }
    Ok(report)
}

fn open(path: &Path) -> Result<ZipArchive<BufReader<File>>, String> {
    let file = File::open(path).map_err(|error| format!("could not open the file: {error}"))?;
    ZipArchive::new(BufReader::new(file)).map_err(|_| not_a_honk_file())
}

fn read_manifest<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Manifest, String> {
    let bytes = read_entry(archive, MANIFEST, MAX_MANIFEST_BYTES).map_err(|_| not_a_honk_file())?;

    let header: Header = serde_json::from_slice(&bytes).map_err(|_| not_a_honk_file())?;
    if header.format.as_deref() != Some(FORMAT) {
        return Err(not_a_honk_file());
    }
    match header.version {
        Some(VERSION) => {}
        Some(version) if version > VERSION => {
            let made_with = header
                .app_version
                .map(|version| format!(" (Honk {version})"))
                .unwrap_or_default();
            return Err(format!(
                "this file was made by a newer version of Honk{made_with}; update Honk to open it"
            ));
        }
        _ => return Err(not_a_honk_file()),
    }

    let mut manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|error| format!("the file is damaged: {error}"))?;
    validate(&mut manifest)?;
    Ok(manifest)
}

/// Checks what can be checked without the audio, and tidies what's harmless to tidy.
fn validate(manifest: &mut Manifest) -> Result<(), String> {
    if manifest.sounds.len() > MAX_SOUNDS {
        return Err(format!("the file has more than {MAX_SOUNDS} sounds"));
    }
    manifest.categories = manifest
        .categories
        .iter()
        .map(|name| clean_name(name))
        .filter(|name| !name.is_empty())
        .collect();
    let categories: HashSet<String> = manifest
        .categories
        .iter()
        .map(|name| name.to_lowercase())
        .collect();

    for sound in &mut manifest.sounds {
        let valid_hash = sound.sha256.len() == 64
            && sound
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        let valid_extension = (1..=8).contains(&sound.extension.len())
            && sound
                .extension
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
        if !valid_hash || !valid_extension {
            return Err("the file is damaged: a sound has an invalid name".to_string());
        }

        sound.name = clean_name(&sound.name);
        if sound.name.is_empty() {
            sound.name = "Untitled".to_string();
        }
        sound.volume = if sound.volume.is_finite() {
            sound.volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        // A category the file doesn't list, or a hotkey Honk can't use, is dropped, not fatal.
        sound.category = sound
            .category
            .as_deref()
            .map(clean_name)
            .filter(|name| categories.contains(&name.to_lowercase()));
        sound.hotkey = sound
            .hotkey
            .as_deref()
            .and_then(|hotkey| hotkeys::normalize(hotkey).ok());
    }
    Ok(())
}

/// Reads each sound's audio in manifest order, checking its size, hash and that it decodes, and
/// hands it to `visit`.
fn for_each_sound<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    manifest: &Manifest,
    mut visit: impl FnMut(&ManifestSound, Vec<u8>) -> Result<(), String>,
) -> Result<(), String> {
    let mut total = 0u64;
    for sound in &manifest.sounds {
        let bytes = read_entry(archive, &sound.entry_name(), MAX_SOUND_BYTES).map_err(|error| {
            format!("\u{201c}{}\u{201d} can't be imported: {error}", sound.name)
        })?;
        total += bytes.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err("the file's sounds add up to more than 1 GB".to_string());
        }
        if crate::library::hash(&bytes) != sound.sha256 {
            return Err(format!(
                "the file is damaged: \u{201c}{}\u{201d} doesn't match its checksum",
                sound.name
            ));
        }
        let data: crate::audio_engine::SoundData = bytes.into();
        crate::audio_engine::decode(&data).map_err(|error| {
            format!("\u{201c}{}\u{201d} can't be imported: {error}", sound.name)
        })?;
        visit(sound, data.to_vec())?;
    }
    Ok(())
}

/// Reads one entry by exact name, refusing more than `limit` bytes whatever its header claims.
fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let entry = archive.by_name(name).map_err(|error| match error {
        zip::result::ZipError::FileNotFound => "it's missing from the file".to_string(),
        error => archive_error(error),
    })?;
    if entry.size() > limit {
        return Err(format!("it's larger than {} MB", limit / (1024 * 1024)));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    // The declared size can lie; the read is capped regardless.
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("it couldn't be read: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("it's larger than {} MB", limit / (1024 * 1024)));
    }
    Ok(bytes)
}

/// Trims, drops control characters, and caps the length, so a name from a file can't be blank,
/// sneak in a newline, or be a megabyte long.
fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|character| !character.is_control())
        .take(MAX_NAME_CHARACTERS)
        .collect::<String>()
        .trim()
        .to_string()
}

fn partial_path(destination: &Path) -> PathBuf {
    let mut name = destination
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".partial");
    destination.with_file_name(name)
}

fn not_a_honk_file() -> String {
    "this isn't a Honk library file".to_string()
}

fn archive_error(error: zip::result::ZipError) -> String {
    format!("the file couldn't be read or written: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::tests::{scratch_directory, wav};

    /// A library with these sounds imported, and where it lives.
    fn library_with(sounds: &[(&str, Vec<u8>)]) -> (Library, PathBuf) {
        let directory = scratch_directory();
        let library = Library::open(&directory.join("library")).unwrap();
        let paths = sounds
            .iter()
            .map(|(name, bytes)| {
                let path = directory.join(name);
                fs::write(&path, bytes).unwrap();
                path
            })
            .collect();
        library.import(paths, None);
        (library, directory)
    }

    fn id_of(library: &Library, name: &str) -> i64 {
        library
            .list()
            .unwrap()
            .into_iter()
            .find(|sound| sound.name == name)
            .unwrap()
            .id
    }

    fn named(library: &Library, name: &str) -> Sound {
        library.get(id_of(library, name)).unwrap()
    }

    /// Writes a zip by hand, for files Honk itself would never write.
    fn raw_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let mut archive = ZipWriter::new(File::create(path).unwrap());
        for (name, bytes) in entries {
            archive
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
    }

    fn manifest_json(sounds: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "format": "honk", "version": 1, "app_version": "0.1.0",
            "categories": ["Memes"], "sounds": sounds,
        }))
        .unwrap()
    }

    #[test]
    fn a_whole_library_survives_the_round_trip() {
        let (source, directory) = library_with(&[
            ("horn.wav", wav(&[1])),
            ("bruh.wav", wav(&[2])),
            ("wow.wav", wav(&[3])),
        ]);
        let memes = source.create_category("Memes").unwrap();
        source.create_category("Empty").unwrap();
        let bruh = id_of(&source, "bruh");
        source.set_category(bruh, Some(memes.id)).unwrap();
        source.set_volume(bruh, 0.4).unwrap();
        source.set_favorite(bruh, true).unwrap();
        source.set_hotkey(bruh, Some("alt+Digit2")).unwrap();
        let (wow, horn) = (id_of(&source, "wow"), id_of(&source, "horn"));
        source.reorder(&[wow, bruh, horn]).unwrap();

        let file = directory.join("board.honk");
        let summary = export(&source, None, &file).unwrap();
        assert_eq!((summary.sounds, summary.categories), (3, 2));
        assert!(!directory.join("board.honk.partial").exists());

        let (target, _) = library_with(&[]);
        let report = import(&target, &file, true, &HashSet::new()).unwrap();
        assert_eq!((report.added, report.duplicates), (3, 0));
        assert_eq!(report.hotkeys_assigned, 1);
        assert_eq!(report.lands_in, None, "spread over a category and none");

        let names: Vec<_> = target.list().unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["wow", "bruh", "horn"]);
        let imported = named(&target, "bruh");
        assert_eq!(imported.volume, 0.4);
        assert!(imported.favorite);
        assert_eq!(imported.hotkey.as_deref(), Some("alt+Digit2"));
        let categories: Vec<_> = target
            .categories()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(categories, ["Memes", "Empty"]);
        assert_eq!(named(&target, "horn").category_id, None);
    }

    #[test]
    fn a_category_merges_into_one_with_the_same_name_and_the_view_follows_it() {
        let (source, directory) = library_with(&[("horn.wav", wav(&[1])), ("clap.wav", wav(&[2]))]);
        let memes = source.create_category("Memes").unwrap();
        source
            .set_category(id_of(&source, "horn"), Some(memes.id))
            .unwrap();
        let file = directory.join("memes.honk");
        assert_eq!(export(&source, Some(memes.id), &file).unwrap().sounds, 1);

        let (target, _) = library_with(&[]);
        let theirs = target.create_category("memes").unwrap();
        let preview = preview(&target, &file).unwrap();
        assert_eq!(preview.categories.len(), 1);
        assert!(preview.categories[0].merges);
        assert_eq!(preview.categories[0].sounds, 1);
        assert_eq!(preview.uncategorized, 0);

        let report = import(&target, &file, false, &HashSet::new()).unwrap();
        assert_eq!(report.lands_in, Some(theirs.id));
        assert_eq!(target.categories().unwrap().len(), 1);
        assert_eq!(named(&target, "horn").category_id, Some(theirs.id));
    }

    #[test]
    fn sounds_already_in_the_library_are_left_exactly_as_they_are() {
        let (source, directory) = library_with(&[("horn.wav", wav(&[1]))]);
        source.set_volume(id_of(&source, "horn"), 0.2).unwrap();
        let file = directory.join("board.honk");
        export(&source, None, &file).unwrap();

        let (target, _) = library_with(&[("my horn.wav", wav(&[1]))]);
        assert_eq!(preview(&target, &file).unwrap().duplicates, 1);
        let report = import(&target, &file, true, &HashSet::new()).unwrap();
        assert_eq!((report.added, report.duplicates), (0, 1));
        let mine = named(&target, "my horn");
        assert_eq!(mine.volume, 1.0);
        assert_eq!(target.list().unwrap().len(), 1);
    }

    #[test]
    fn hotkeys_are_opt_in_and_a_taken_one_is_skipped_unless_replaced() {
        let (source, directory) = library_with(&[("horn.wav", wav(&[1]))]);
        source
            .set_hotkey(id_of(&source, "horn"), Some("alt+Digit1"))
            .unwrap();
        let file = directory.join("board.honk");
        export(&source, None, &file).unwrap();
        let replace: HashSet<String> = ["alt+Digit1".to_string()].into();

        let (not_asked, _) = library_with(&[]);
        let report = import(&not_asked, &file, false, &replace).unwrap();
        assert_eq!((report.hotkeys_assigned, report.hotkeys_skipped), (0, 1));
        assert_eq!(named(&not_asked, "horn").hotkey, None);

        let (skipping, _) = library_with(&[("mine.wav", wav(&[9]))]);
        skipping
            .set_hotkey(id_of(&skipping, "mine"), Some("alt+Digit1"))
            .unwrap();
        let preview = preview(&skipping, &file).unwrap();
        assert_eq!(
            preview.hotkeys[0].taken_by.as_deref(),
            Some("\u{201c}mine\u{201d}")
        );
        let report = import(&skipping, &file, true, &HashSet::new()).unwrap();
        assert_eq!(report.hotkeys_skipped, 1);
        assert_eq!(
            named(&skipping, "mine").hotkey.as_deref(),
            Some("alt+Digit1")
        );
        assert_eq!(named(&skipping, "horn").hotkey, None);

        let (replacing, _) = library_with(&[("mine.wav", wav(&[9]))]);
        replacing
            .set_hotkey(id_of(&replacing, "mine"), Some("alt+Digit1"))
            .unwrap();
        let report = import(&replacing, &file, true, &replace).unwrap();
        assert_eq!(report.hotkeys_assigned, 1);
        assert_eq!(named(&replacing, "mine").hotkey, None);
        assert_eq!(
            named(&replacing, "horn").hotkey.as_deref(),
            Some("alt+Digit1")
        );
    }

    #[test]
    fn a_damaged_sound_stops_the_whole_import_before_anything_is_added() {
        let directory = scratch_directory();
        let good = wav(&[1]);
        let bad = wav(&[2]);
        let manifest = manifest_json(serde_json::json!([
            { "name": "good", "sha256": crate::library::hash(&good), "extension": "wav",
              "volume": 1.0, "favorite": false, "hotkey": null, "category": null },
            { "name": "bad", "sha256": crate::library::hash(&wav(&[3])), "extension": "wav",
              "volume": 1.0, "favorite": false, "hotkey": null, "category": null },
        ]));
        let file = directory.join("damaged.honk");
        let good_entry = format!("sounds/{}.wav", crate::library::hash(&good));
        let bad_entry = format!("sounds/{}.wav", crate::library::hash(&wav(&[3])));
        raw_archive(
            &file,
            &[
                ("manifest.json", &manifest),
                (&good_entry, &good),
                (&bad_entry, &bad),
            ],
        );

        let (target, _) = library_with(&[]);
        let error = import(&target, &file, false, &HashSet::new()).unwrap_err();
        assert!(error.contains("checksum"), "{error}");
        assert!(target.list().unwrap().is_empty());
        assert!(target.categories().unwrap().is_empty());
    }

    #[test]
    fn files_that_are_not_honk_files_or_are_from_a_newer_honk_are_refused_clearly() {
        let directory = scratch_directory();
        let (target, _) = library_with(&[]);

        let not_zip = directory.join("notes.honk");
        fs::write(&not_zip, b"hello").unwrap();
        assert!(preview(&target, &not_zip)
            .unwrap_err()
            .contains("isn't a Honk library"));

        let other_zip = directory.join("photos.honk");
        raw_archive(&other_zip, &[("photo.jpg", b"not really")]);
        assert!(preview(&target, &other_zip)
            .unwrap_err()
            .contains("isn't a Honk library"));

        let newer = directory.join("newer.honk");
        let manifest = br#"{"format":"honk","version":2,"app_version":"3.0.0","shape":"new"}"#;
        raw_archive(&newer, &[("manifest.json", manifest)]);
        let error = preview(&target, &newer).unwrap_err();
        assert!(
            error.contains("newer version of Honk (Honk 3.0.0)"),
            "{error}"
        );
    }

    #[test]
    fn paths_inside_the_archive_are_never_used_and_its_text_is_cleaned() {
        let directory = scratch_directory();
        let audio = wav(&[5]);
        let hash = crate::library::hash(&audio);
        let manifest = manifest_json(serde_json::json!([
            { "name": "  line\nbreak  ", "sha256": hash, "extension": "wav", "volume": 7.0,
              "favorite": true, "hotkey": "not a hotkey", "category": "Unlisted" },
        ]));
        let entry = format!("sounds/{hash}.wav");
        let file = directory.join("sneaky.honk");
        raw_archive(
            &file,
            &[
                ("manifest.json", &manifest),
                (&entry, &audio),
                ("../../escaped.txt", b"should never be written"),
            ],
        );

        let (target, library_directory) = library_with(&[]);
        let report = import(&target, &file, true, &HashSet::new()).unwrap();
        assert_eq!(report.added, 1);
        let sound = named(&target, "linebreak");
        assert_eq!(sound.volume, 1.0);
        assert_eq!(sound.hotkey, None);
        assert_eq!(
            sound.category_id, None,
            "a category the file doesn't list is dropped"
        );
        assert!(!directory.join("escaped.txt").exists());
        assert!(!library_directory.join("escaped.txt").exists());
    }

    #[test]
    fn an_entry_larger_than_its_limit_is_refused_whatever_its_header_says() {
        let directory = scratch_directory();
        let file = directory.join("big.honk");
        raw_archive(&file, &[("sounds/big.wav", &[0u8; 100])]);
        let mut archive = open(&file).unwrap();
        assert!(read_entry(&mut archive, "sounds/big.wav", 10)
            .unwrap_err()
            .contains("larger than"));
        assert_eq!(
            read_entry(&mut archive, "sounds/big.wav", 100)
                .unwrap()
                .len(),
            100
        );
    }
}
