//! The sound library: imported audio files copied into app storage, and their metadata in SQLite.
//!
//! Stored files are named after the SHA-256 of their contents, so importing the same audio twice
//! stores it once, and moving or deleting the original never breaks a pad.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::audio_engine::{self, SoundData};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sound {
    pub id: i64,
    pub name: String,
    /// Linear, 0 to 1.
    pub volume: f32,
    pub favorite: bool,
    /// Canonical global-shortcut string, e.g. `alt+Digit1`. See `hotkeys::normalize`.
    pub hotkey: Option<String>,
    /// The one category the sound is in, if any.
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub path: PathBuf,
    /// The imported sound, or the existing one when the file was already in the library.
    pub sound: Option<Sound>,
    pub duplicate: bool,
    pub error: Option<String>,
}

pub struct Library {
    connection: Mutex<Connection>,
    sounds_directory: PathBuf,
}

/// Each entry upgrades the schema by one version; `PRAGMA user_version` records how many ran.
/// Never edit a released migration — append a new one.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE sounds (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        content_hash TEXT NOT NULL UNIQUE,
        file_name TEXT NOT NULL,
        volume REAL NOT NULL DEFAULT 1.0,
        favorite INTEGER NOT NULL DEFAULT 0,
        position INTEGER NOT NULL,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );",
    "ALTER TABLE sounds ADD COLUMN hotkey TEXT;
     CREATE UNIQUE INDEX sounds_hotkey ON sounds (hotkey) WHERE hotkey IS NOT NULL;
     CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    // Foreign keys aren't enforced (SQLite leaves them off by default), so `delete_category`
    // clears the column itself.
    "CREATE TABLE categories (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE
     );
     ALTER TABLE sounds ADD COLUMN category_id INTEGER REFERENCES categories (id);",
];

/// Every query that builds a `Sound` selects these, in this order — see `sound_from_row`.
const SOUND_COLUMNS: &str = "id, name, volume, favorite, hotkey, category_id";

/// App-wide global shortcuts, as opposed to one per pad. Each is stored under its own
/// `settings` key and can't share a hotkey with a pad or with another app shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppShortcut {
    StopAll,
    TogglePopover,
}

/// The `settings` key for whether Honk shows in the Dock (macOS). Absent means yes.
const SHOW_IN_DOCK: &str = "show_in_dock";

impl AppShortcut {
    pub const ALL: [AppShortcut; 2] = [AppShortcut::StopAll, AppShortcut::TogglePopover];

    fn setting_key(self) -> &'static str {
        match self {
            // Named before there was more than one; renaming it would drop saved hotkeys.
            AppShortcut::StopAll => "stop_all_hotkey",
            AppShortcut::TogglePopover => "toggle_popover_hotkey",
        }
    }

    fn label(self) -> &'static str {
        match self {
            AppShortcut::StopAll => "Stop all",
            AppShortcut::TogglePopover => "the popover",
        }
    }
}

impl Library {
    /// Opens (or creates) the library under `directory`: `library.sqlite3` plus a `sounds/`
    /// folder for the stored files.
    pub fn open(directory: &Path) -> Result<Self, String> {
        let sounds_directory = directory.join("sounds");
        fs::create_dir_all(&sounds_directory)
            .map_err(|error| format!("could not create {}: {error}", sounds_directory.display()))?;
        let connection = Connection::open(directory.join("library.sqlite3"))
            .map_err(|error| format!("could not open the library database: {error}"))?;
        Self::from_connection(connection, sounds_directory)
    }

    fn from_connection(
        mut connection: Connection,
        sounds_directory: PathBuf,
    ) -> Result<Self, String> {
        migrate(&mut connection)
            .map_err(|error| format!("could not migrate the library: {error}"))?;
        Ok(Self {
            connection: Mutex::new(connection),
            sounds_directory,
        })
    }

    pub fn list(&self) -> Result<Vec<Sound>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {SOUND_COLUMNS} FROM sounds ORDER BY position"
            ))
            .map_err(database_error)?;
        let sounds = statement
            .query_map([], sound_from_row)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        Ok(sounds)
    }

    /// Imports each path independently — one bad file doesn't stop the rest. New sounds go in
    /// `category`; a file that's already in the library keeps the category it has.
    pub fn import(&self, paths: Vec<PathBuf>, category: Option<i64>) -> Vec<ImportResult> {
        paths
            .into_iter()
            .map(|path| match self.import_one(&path, category) {
                Ok((sound, duplicate)) => ImportResult {
                    path,
                    sound: Some(sound),
                    duplicate,
                    error: None,
                },
                Err(error) => ImportResult {
                    path,
                    sound: None,
                    duplicate: false,
                    error: Some(error),
                },
            })
            .collect()
    }

    fn import_one(&self, path: &Path, category: Option<i64>) -> Result<(Sound, bool), String> {
        let bytes = fs::read(path).map_err(|error| format!("could not read the file: {error}"))?;
        let content_hash = hash(&bytes);

        let connection = self.connection()?;
        if let Some(existing) = find_by_hash(&connection, &content_hash)? {
            return Ok((existing, true));
        }

        // Reject anything that won't play before it takes up space in the library.
        let sound_data: SoundData = bytes.into();
        audio_engine::decode(&sound_data)?;

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .unwrap_or_else(|| "audio".to_string());
        let file_name = format!("{content_hash}.{extension}");
        let stored_path = self.sounds_directory.join(&file_name);
        fs::write(&stored_path, &sound_data)
            .map_err(|error| format!("could not store the file: {error}"))?;

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let inserted = connection
            .query_row(
                &format!(
                    // A category deleted since the caller listed them becomes no category.
                    "INSERT INTO sounds (name, content_hash, file_name, position, category_id)
                     VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(position), -1) + 1 FROM sounds),
                             (SELECT id FROM categories WHERE id = ?4))
                     RETURNING {SOUND_COLUMNS}"
                ),
                params![name, content_hash, file_name, category],
                sound_from_row,
            )
            .map_err(database_error);

        if inserted.is_err() {
            // Don't leave an orphaned file behind a failed insert.
            let _ = fs::remove_file(&stored_path);
        }
        Ok((inserted?, false))
    }

    /// The stored file's contents, for playback.
    pub fn load(&self, id: i64) -> Result<SoundData, String> {
        let file_name = self.file_name(id)?;
        let path = self.sounds_directory.join(file_name);
        fs::read(&path)
            .map(Into::into)
            .map_err(|error| format!("could not read the stored sound: {error}"))
    }

    pub fn rename(&self, id: i64, name: &str) -> Result<Sound, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a sound needs a name".to_string());
        }
        self.update(id, "UPDATE sounds SET name = ?2 WHERE id = ?1", name)
    }

    pub fn set_volume(&self, id: i64, volume: f32) -> Result<Sound, String> {
        self.update(
            id,
            "UPDATE sounds SET volume = ?2 WHERE id = ?1",
            volume.clamp(0.0, 1.0),
        )
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<Sound, String> {
        self.update(
            id,
            "UPDATE sounds SET favorite = ?2 WHERE id = ?1",
            favorite,
        )
    }

    /// Puts a sound in `category`, or in none with `None`.
    pub fn set_category(&self, id: i64, category: Option<i64>) -> Result<Sound, String> {
        if let Some(category) = category {
            let exists = self
                .connection()?
                .query_row("SELECT 1 FROM categories WHERE id = ?1", [category], |_| {
                    Ok(())
                })
                .optional()
                .map_err(database_error)?
                .is_some();
            if !exists {
                return Err("that category no longer exists".to_string());
            }
        }
        self.update(
            id,
            "UPDATE sounds SET category_id = ?2 WHERE id = ?1",
            category,
        )
    }

    /// Every category, oldest first.
    pub fn categories(&self) -> Result<Vec<Category>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT id, name FROM categories ORDER BY id")
            .map_err(database_error)?;
        let categories = statement
            .query_map([], category_from_row)
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        Ok(categories)
    }

    /// Names are trimmed and unique, ignoring case.
    pub fn create_category(&self, name: &str) -> Result<Category, String> {
        let name = category_name(name)?;
        self.connection()?
            .query_row(
                "INSERT INTO categories (name) VALUES (?1) RETURNING id, name",
                [name],
                category_from_row,
            )
            .map_err(|error| category_error(error, name))
    }

    pub fn rename_category(&self, id: i64, name: &str) -> Result<Category, String> {
        let name = category_name(name)?;
        self.connection()?
            .query_row(
                "UPDATE categories SET name = ?2 WHERE id = ?1 RETURNING id, name",
                params![id, name],
                category_from_row,
            )
            .optional()
            .map_err(|error| category_error(error, name))?
            .ok_or_else(|| "that category no longer exists".to_string())
    }

    /// Removes the category. Its sounds stay in the library, in no category.
    pub fn delete_category(&self, id: i64) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(database_error)?;
        transaction
            .execute(
                "UPDATE sounds SET category_id = NULL WHERE category_id = ?1",
                [id],
            )
            .map_err(database_error)?;
        let deleted = transaction
            .execute("DELETE FROM categories WHERE id = ?1", [id])
            .map_err(database_error)?;
        if deleted == 0 {
            return Err("that category no longer exists".to_string());
        }
        transaction.commit().map_err(database_error)
    }

    /// Puts the library in the order of `ids`, which must name every sound exactly once. A list
    /// that doesn't — the library changed since the caller last listed it — is refused, and the
    /// order is left as it was.
    pub fn reorder(&self, ids: &[i64]) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(database_error)?;
        let mut current = transaction
            .prepare("SELECT id FROM sounds")
            .map_err(database_error)?
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        let mut requested = ids.to_vec();
        current.sort_unstable();
        requested.sort_unstable();
        if current != requested {
            return Err("the library changed while reordering; try again".to_string());
        }
        {
            let mut statement = transaction
                .prepare("UPDATE sounds SET position = ?2 WHERE id = ?1")
                .map_err(database_error)?;
            for (position, id) in ids.iter().enumerate() {
                statement
                    .execute(params![id, position as i64])
                    .map_err(database_error)?;
            }
        }
        transaction.commit().map_err(database_error)
    }

    /// Removes the sound and its stored file.
    pub fn delete(&self, id: i64) -> Result<(), String> {
        let file_name = self.file_name(id)?;
        self.connection()?
            .execute("DELETE FROM sounds WHERE id = ?1", [id])
            .map_err(database_error)?;
        match fs::remove_file(self.sounds_directory.join(file_name)) {
            Ok(()) => Ok(()),
            // Already gone is the outcome we wanted.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("removed the sound, but not its file: {error}")),
        }
    }

    /// Assigns `hotkey` (already normalized) to a sound, or clears it with `None`. Refuses a
    /// hotkey another pad or an app shortcut already uses, naming which.
    pub fn set_hotkey(&self, id: i64, hotkey: Option<&str>) -> Result<Sound, String> {
        if let Some(hotkey) = hotkey {
            let connection = self.connection()?;
            if let Some(owner) = hotkey_owner(&connection, hotkey, Holder::Sound(id))? {
                return Err(format!("that shortcut is already used by {owner}"));
            }
        }
        self.update(id, "UPDATE sounds SET hotkey = ?2 WHERE id = ?1", hotkey)
    }

    pub fn app_hotkey(&self, shortcut: AppShortcut) -> Result<Option<String>, String> {
        let connection = self.connection()?;
        read_setting(&connection, shortcut.setting_key())
    }

    /// Sets or clears an app shortcut, refusing a hotkey a pad or another app shortcut uses.
    pub fn set_app_hotkey(
        &self,
        shortcut: AppShortcut,
        hotkey: Option<&str>,
    ) -> Result<(), String> {
        let connection = self.connection()?;
        match hotkey {
            Some(hotkey) => {
                if let Some(owner) = hotkey_owner(&connection, hotkey, Holder::App(shortcut))? {
                    return Err(format!("that shortcut is already used by {owner}"));
                }
                connection
                    .execute(
                        "INSERT INTO settings (key, value) VALUES (?1, ?2)
                         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                        params![shortcut.setting_key(), hotkey],
                    )
                    .map_err(database_error)?;
            }
            None => {
                connection
                    .execute(
                        "DELETE FROM settings WHERE key = ?1",
                        [shortcut.setting_key()],
                    )
                    .map_err(database_error)?;
            }
        }
        Ok(())
    }

    pub fn show_in_dock(&self) -> Result<bool, String> {
        let connection = self.connection()?;
        Ok(read_setting(&connection, SHOW_IN_DOCK)?.as_deref() != Some("false"))
    }

    pub fn set_show_in_dock(&self, show: bool) -> Result<(), String> {
        self.connection()?
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![SHOW_IN_DOCK, show.to_string()],
            )
            .map_err(database_error)?;
        Ok(())
    }

    fn update(
        &self,
        id: i64,
        statement: &str,
        value: impl rusqlite::ToSql,
    ) -> Result<Sound, String> {
        let connection = self.connection()?;
        let changed = connection
            .execute(statement, params![id, value])
            .map_err(database_error)?;
        if changed == 0 {
            return Err(not_found(id));
        }
        find_by_id(&connection, id)?.ok_or_else(|| not_found(id))
    }

    pub fn get(&self, id: i64) -> Result<Sound, String> {
        let connection = self.connection()?;
        find_by_id(&connection, id)?.ok_or_else(|| not_found(id))
    }

    fn file_name(&self, id: i64) -> Result<String, String> {
        self.connection()?
            .query_row("SELECT file_name FROM sounds WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| not_found(id))
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "the library database lock is poisoned".to_string())
    }
}

fn migrate(connection: &mut Connection) -> rusqlite::Result<()> {
    let applied: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (version, migration) in (1..).zip(MIGRATIONS).skip(applied.max(0) as usize) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", version as i64)?;
        transaction.commit()?;
    }
    Ok(())
}

/// Whatever is asking for a hotkey, so it isn't reported as conflicting with itself.
#[derive(Clone, Copy)]
enum Holder {
    Sound(i64),
    App(AppShortcut),
}

/// Who, other than `asking`, already uses `hotkey` — described for an error message.
fn hotkey_owner(
    connection: &Connection,
    hotkey: &str,
    asking: Holder,
) -> Result<Option<String>, String> {
    let excluded_sound = match asking {
        Holder::Sound(id) => id,
        Holder::App(_) => -1,
    };
    let sound = connection
        .query_row(
            "SELECT name FROM sounds WHERE hotkey = ?1 AND id != ?2",
            params![hotkey, excluded_sound],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(database_error)?;
    if let Some(name) = sound {
        return Ok(Some(format!("\u{201c}{name}\u{201d}")));
    }
    for shortcut in AppShortcut::ALL {
        if matches!(asking, Holder::App(own) if own == shortcut) {
            continue;
        }
        if read_setting(connection, shortcut.setting_key())?.as_deref() == Some(hotkey) {
            return Ok(Some(shortcut.label().to_string()));
        }
    }
    Ok(None)
}

fn read_setting(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(database_error)
}

fn find_by_id(connection: &Connection, id: i64) -> Result<Option<Sound>, String> {
    connection
        .query_row(
            &format!("SELECT {SOUND_COLUMNS} FROM sounds WHERE id = ?1"),
            [id],
            sound_from_row,
        )
        .optional()
        .map_err(database_error)
}

fn find_by_hash(connection: &Connection, content_hash: &str) -> Result<Option<Sound>, String> {
    connection
        .query_row(
            &format!("SELECT {SOUND_COLUMNS} FROM sounds WHERE content_hash = ?1"),
            [content_hash],
            sound_from_row,
        )
        .optional()
        .map_err(database_error)
}

fn sound_from_row(row: &Row) -> rusqlite::Result<Sound> {
    Ok(Sound {
        id: row.get(0)?,
        name: row.get(1)?,
        volume: row.get(2)?,
        favorite: row.get(3)?,
        hotkey: row.get(4)?,
        category_id: row.get(5)?,
    })
}

fn category_from_row(row: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
    })
}

fn category_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a category needs a name".to_string());
    }
    Ok(name)
}

/// Explains a clash with another category's name; anything else is a plain database error.
fn category_error(error: rusqlite::Error, name: &str) -> String {
    match error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            format!("there's already a category called \u{201c}{name}\u{201d}")
        }
        error => database_error(error),
    }
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn database_error(error: rusqlite::Error) -> String {
    format!("library database error: {error}")
}

fn not_found(id: i64) -> String {
    format!("sound {id} is not in the library")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fresh directory per test, so tests can run in parallel without sharing files.
    fn scratch_directory() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "honk-library-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    /// A minimal 16-bit mono PCM WAV; different `samples` give different content hashes.
    fn wav(samples: &[i16]) -> Vec<u8> {
        let data_length = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44_100u32.to_le_bytes());
        bytes.extend_from_slice(&88_200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    fn library_with_files(files: &[(&str, Vec<u8>)]) -> (Library, PathBuf, Vec<PathBuf>) {
        let directory = scratch_directory();
        let library = Library::open(&directory.join("library")).unwrap();
        let paths = files
            .iter()
            .map(|(name, bytes)| {
                let path = directory.join(name);
                fs::write(&path, bytes).unwrap();
                path
            })
            .collect();
        (library, directory, paths)
    }

    fn stored_files(directory: &Path) -> usize {
        fs::read_dir(directory.join("library/sounds"))
            .unwrap()
            .count()
    }

    #[test]
    fn imports_a_sound_named_after_its_file_and_stored_by_hash() {
        let (library, directory, paths) = library_with_files(&[("Airhorn.WAV", wav(&[1, 2, 3]))]);
        let results = library.import(paths, None);

        let sound = results[0].sound.clone().expect("import should succeed");
        assert_eq!(sound.name, "Airhorn");
        assert_eq!(sound.volume, 1.0);
        assert!(!sound.favorite);
        assert!(!results[0].duplicate);

        let stored = fs::read_dir(directory.join("library/sounds"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        assert_eq!(
            stored.to_str().unwrap(),
            format!("{}.wav", hash(&wav(&[1, 2, 3])))
        );
    }

    #[test]
    fn the_same_audio_twice_is_stored_once_even_under_another_name() {
        let (library, directory, paths) = library_with_files(&[
            ("first.wav", wav(&[1, 2, 3])),
            ("renamed copy.wav", wav(&[1, 2, 3])),
        ]);
        let results = library.import(paths, None);

        assert!(!results[0].duplicate);
        assert!(results[1].duplicate);
        assert_eq!(results[0].sound, results[1].sound);
        assert_eq!(library.list().unwrap().len(), 1);
        assert_eq!(stored_files(&directory), 1);
    }

    #[test]
    fn a_file_that_is_not_audio_is_rejected_without_storing_anything() {
        let (library, directory, paths) = library_with_files(&[
            ("notes.mp3", b"not audio at all".to_vec()),
            ("real.wav", wav(&[4, 5, 6])),
        ]);
        let results = library.import(paths, None);

        assert!(results[0].error.is_some());
        assert!(
            results[1].sound.is_some(),
            "one bad file must not stop the rest"
        );
        assert_eq!(library.list().unwrap().len(), 1);
        assert_eq!(stored_files(&directory), 1);
    }

    #[test]
    fn sounds_list_in_import_order() {
        let (library, _, paths) = library_with_files(&[
            ("c.wav", wav(&[3])),
            ("a.wav", wav(&[1])),
            ("b.wav", wav(&[2])),
        ]);
        library.import(paths, None);
        let names: Vec<_> = library
            .list()
            .unwrap()
            .into_iter()
            .map(|sound| sound.name)
            .collect();
        assert_eq!(names, ["c", "a", "b"]);
    }

    fn names(library: &Library) -> Vec<String> {
        library
            .list()
            .unwrap()
            .into_iter()
            .map(|sound| sound.name)
            .collect()
    }

    #[test]
    fn reordering_saves_the_new_order_and_new_imports_go_last() {
        let (library, directory, paths) = library_with_files(&[
            ("a.wav", wav(&[1])),
            ("b.wav", wav(&[2])),
            ("c.wav", wav(&[3])),
        ]);
        let ids: Vec<_> = library
            .import(paths, None)
            .into_iter()
            .map(|result| result.sound.unwrap().id)
            .collect();

        library.reorder(&[ids[2], ids[0], ids[1]]).unwrap();
        assert_eq!(names(&library), ["c", "a", "b"]);

        let later = directory.join("d.wav");
        fs::write(&later, wav(&[4])).unwrap();
        library.import(vec![later], None);
        assert_eq!(names(&library), ["c", "a", "b", "d"]);
    }

    #[test]
    fn a_reorder_that_does_not_match_the_library_changes_nothing() {
        let (library, _, paths) = library_with_files(&[("a.wav", wav(&[1])), ("b.wav", wav(&[2]))]);
        let ids: Vec<_> = library
            .import(paths, None)
            .into_iter()
            .map(|result| result.sound.unwrap().id)
            .collect();

        for stale in [
            vec![ids[1]],
            vec![ids[1], ids[0], ids[0] + ids[1]],
            vec![ids[1], ids[1]],
        ] {
            assert!(
                library.reorder(&stale).is_err(),
                "{stale:?} should be refused"
            );
            assert_eq!(names(&library), ["a", "b"]);
        }
    }

    #[test]
    fn categories_are_named_uniquely_and_listed_oldest_first() {
        let (library, _, _) = library_with_files(&[]);
        let memes = library.create_category("  Memes ").unwrap();
        assert_eq!(memes.name, "Memes");
        library.create_category("Stream").unwrap();

        assert!(library
            .create_category("MEMES")
            .unwrap_err()
            .contains("already a category"));
        assert!(library.create_category("   ").is_err());
        assert!(library
            .rename_category(memes.id, "stream")
            .unwrap_err()
            .contains("already a category"));
        assert_eq!(
            library.rename_category(memes.id, "Bits").unwrap().name,
            "Bits"
        );
        assert!(library.rename_category(memes.id + 100, "Nope").is_err());

        let names: Vec<_> = library
            .categories()
            .unwrap()
            .into_iter()
            .map(|category| category.name)
            .collect();
        assert_eq!(names, ["Bits", "Stream"]);
    }

    #[test]
    fn new_imports_go_in_the_given_category_but_duplicates_keep_theirs() {
        let (library, directory, paths) = library_with_files(&[("horn.wav", wav(&[1]))]);
        let memes = library.create_category("Memes").unwrap();
        let stream = library.create_category("Stream").unwrap();

        let horn = library.import(paths.clone(), Some(memes.id))[0]
            .sound
            .clone()
            .unwrap();
        assert_eq!(horn.category_id, Some(memes.id));

        let again = library.import(paths, Some(stream.id)).remove(0);
        assert!(again.duplicate);
        assert_eq!(again.sound.unwrap().category_id, Some(memes.id));

        // A category deleted in the meantime means no category, not a dangling one.
        let later = directory.join("clap.wav");
        fs::write(&later, wav(&[2])).unwrap();
        let clap = library.import(vec![later], Some(stream.id + 100))[0]
            .sound
            .clone()
            .unwrap();
        assert_eq!(clap.category_id, None);
    }

    #[test]
    fn deleting_a_category_keeps_its_sounds_in_no_category() {
        let (library, _, paths) = library_with_files(&[("horn.wav", wav(&[1]))]);
        let memes = library.create_category("Memes").unwrap();
        let horn = library.import(paths, None)[0].sound.clone().unwrap();

        assert_eq!(
            library
                .set_category(horn.id, Some(memes.id))
                .unwrap()
                .category_id,
            Some(memes.id)
        );
        assert!(library.set_category(horn.id, Some(memes.id + 100)).is_err());

        library.delete_category(memes.id).unwrap();
        assert!(library.categories().unwrap().is_empty());
        assert_eq!(library.get(horn.id).unwrap().category_id, None);
        assert!(library.delete_category(memes.id).is_err());
    }

    #[test]
    fn edits_are_saved_and_clamped() {
        let (library, _, paths) = library_with_files(&[("horn.wav", wav(&[7]))]);
        let id = library.import(paths, None)[0].sound.clone().unwrap().id;

        assert_eq!(library.rename(id, "  Air horn  ").unwrap().name, "Air horn");
        assert!(library.rename(id, "   ").is_err());
        assert_eq!(library.set_volume(id, 1.7).unwrap().volume, 1.0);
        assert_eq!(library.set_volume(id, 0.25).unwrap().volume, 0.25);
        assert!(library.set_favorite(id, true).unwrap().favorite);
        assert_eq!(library.get(id).unwrap().name, "Air horn");
        assert!(library.get(id + 1).is_err());
    }

    #[test]
    fn a_hotkey_is_saved_cleared_and_never_shared() {
        let (library, _, paths) =
            library_with_files(&[("horn.wav", wav(&[11])), ("drum.wav", wav(&[12]))]);
        let results = library.import(paths, None);
        let horn = results[0].sound.clone().unwrap().id;
        let drum = results[1].sound.clone().unwrap().id;

        assert_eq!(
            library
                .set_hotkey(horn, Some("alt+Digit1"))
                .unwrap()
                .hotkey
                .as_deref(),
            Some("alt+Digit1")
        );
        // Re-saving a pad's own hotkey isn't a conflict.
        assert!(library.set_hotkey(horn, Some("alt+Digit1")).is_ok());

        let error = library.set_hotkey(drum, Some("alt+Digit1")).unwrap_err();
        assert!(
            error.contains("horn"),
            "should name the pad that owns it: {error}"
        );

        assert_eq!(library.set_hotkey(horn, None).unwrap().hotkey, None);
        assert!(library.set_hotkey(drum, Some("alt+Digit1")).is_ok());
    }

    #[test]
    fn app_shortcuts_and_pads_never_share_a_hotkey() {
        let (library, _, paths) = library_with_files(&[("horn.wav", wav(&[13]))]);
        let horn = library.import(paths, None)[0].sound.clone().unwrap().id;
        let stop_all = AppShortcut::StopAll;
        let popover = AppShortcut::TogglePopover;
        assert_eq!(library.app_hotkey(stop_all).unwrap(), None);

        library
            .set_app_hotkey(stop_all, Some("alt+Escape"))
            .unwrap();
        assert_eq!(
            library.app_hotkey(stop_all).unwrap().as_deref(),
            Some("alt+Escape")
        );
        // Re-saving its own hotkey isn't a conflict.
        assert!(library.set_app_hotkey(stop_all, Some("alt+Escape")).is_ok());
        assert!(library
            .set_hotkey(horn, Some("alt+Escape"))
            .unwrap_err()
            .contains("Stop all"));
        assert!(library
            .set_app_hotkey(popover, Some("alt+Escape"))
            .unwrap_err()
            .contains("Stop all"));

        library.set_app_hotkey(popover, Some("alt+Space")).unwrap();
        assert!(library
            .set_app_hotkey(stop_all, Some("alt+Space"))
            .unwrap_err()
            .contains("popover"));
        assert!(library
            .set_hotkey(horn, Some("alt+Space"))
            .unwrap_err()
            .contains("popover"));

        library.set_hotkey(horn, Some("alt+Digit2")).unwrap();
        assert!(library
            .set_app_hotkey(popover, Some("alt+Digit2"))
            .unwrap_err()
            .contains("horn"));

        library.set_app_hotkey(stop_all, None).unwrap();
        assert_eq!(library.app_hotkey(stop_all).unwrap(), None);
        assert_eq!(
            library.app_hotkey(popover).unwrap().as_deref(),
            Some("alt+Space")
        );
    }

    #[test]
    fn showing_in_the_dock_defaults_to_yes_and_persists() {
        let directory = scratch_directory();
        let library = Library::open(&directory).unwrap();
        assert!(library.show_in_dock().unwrap());

        library.set_show_in_dock(false).unwrap();
        assert!(!Library::open(&directory).unwrap().show_in_dock().unwrap());

        library.set_show_in_dock(true).unwrap();
        assert!(library.show_in_dock().unwrap());
    }

    #[test]
    fn a_version_one_library_upgrades_and_keeps_its_sounds() {
        let directory = scratch_directory();
        let library_directory = directory.join("library");
        fs::create_dir_all(library_directory.join("sounds")).unwrap();
        {
            // Exactly what a library created before hotkeys existed looks like.
            let connection = Connection::open(library_directory.join("library.sqlite3")).unwrap();
            connection.execute_batch(MIGRATIONS[0]).unwrap();
            connection.pragma_update(None, "user_version", 1).unwrap();
            connection
                .execute(
                    "INSERT INTO sounds (name, content_hash, file_name, position)
                     VALUES ('old', 'abc', 'abc.wav', 0)",
                    [],
                )
                .unwrap();
        }
        let library = Library::open(&library_directory).unwrap();
        let sounds = library.list().unwrap();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].name, "old");
        assert_eq!(sounds[0].hotkey, None);
        assert_eq!(sounds[0].category_id, None);
        assert!(library.categories().unwrap().is_empty());
    }

    #[test]
    fn deleting_removes_the_row_and_the_stored_file() {
        let (library, directory, paths) = library_with_files(&[("horn.wav", wav(&[8]))]);
        let id = library.import(paths, None)[0].sound.clone().unwrap().id;

        library.delete(id).unwrap();
        assert!(library.list().unwrap().is_empty());
        assert_eq!(stored_files(&directory), 0);
        assert!(library.load(id).is_err());
        assert!(library.delete(id).is_err());
    }

    #[test]
    fn the_stored_copy_plays_after_the_original_is_gone() {
        let (library, _, paths) = library_with_files(&[("horn.wav", wav(&[9, 9]))]);
        let id = library.import(paths.clone(), None)[0]
            .sound
            .clone()
            .unwrap()
            .id;
        fs::remove_file(&paths[0]).unwrap();

        let data = library.load(id).unwrap();
        assert!(audio_engine::decode(&data).is_ok());
    }

    #[test]
    fn reopening_keeps_the_library_and_does_not_rerun_migrations() {
        let directory = scratch_directory();
        let source = directory.join("horn.wav");
        fs::write(&source, wav(&[10])).unwrap();
        {
            let library = Library::open(&directory.join("library")).unwrap();
            library.import(vec![source], None);
        }
        let reopened = Library::open(&directory.join("library")).unwrap();
        assert_eq!(reopened.list().unwrap().len(), 1);
    }
}
