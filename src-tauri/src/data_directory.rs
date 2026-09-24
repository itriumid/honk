//! Where Honk keeps its library, and moving it from where older versions kept it.
//!
//! Tauri names the app's data directory after the bundle identifier. Honk 0.1.0 was published
//! as `id.zakir.honk`; later versions are `id.itrium.honk`. Without a move, upgrading would open
//! an empty library while the old one sat untouched next to it.

use std::fs;
use std::io;
use std::path::Path;

/// Bundle identifiers Honk has been published under before, newest first.
const PREVIOUS_IDENTIFIERS: &[&str] = &["id.zakir.honk"];

/// Moves an older version's data directory to `current`, if `current` doesn't exist yet and an
/// older one does. Returns whether it moved anything. An existing `current` is never touched,
/// so this only ever acts on the first run after upgrading.
pub fn adopt_previous(current: &Path) -> io::Result<bool> {
    if current.exists() {
        return Ok(false);
    }
    for identifier in PREVIOUS_IDENTIFIERS {
        let previous = current.with_file_name(identifier);
        if previous.is_dir() {
            fs::rename(&previous, current)?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::tests::scratch_directory;

    #[test]
    fn the_previous_library_moves_to_the_new_directory() {
        let parent = scratch_directory();
        let previous = parent.join("id.zakir.honk");
        fs::create_dir_all(previous.join("sounds")).unwrap();
        fs::write(previous.join("library.sqlite3"), b"library").unwrap();

        let current = parent.join("id.itrium.honk");
        assert!(adopt_previous(&current).unwrap());
        assert_eq!(
            fs::read(current.join("library.sqlite3")).unwrap(),
            b"library"
        );
        assert!(current.join("sounds").is_dir());
        assert!(!previous.exists());
    }

    #[test]
    fn an_existing_library_is_never_replaced() {
        let parent = scratch_directory();
        let previous = parent.join("id.zakir.honk");
        let current = parent.join("id.itrium.honk");
        fs::create_dir_all(&previous).unwrap();
        fs::create_dir_all(&current).unwrap();
        fs::write(current.join("library.sqlite3"), b"new").unwrap();

        assert!(!adopt_previous(&current).unwrap());
        assert_eq!(fs::read(current.join("library.sqlite3")).unwrap(), b"new");
        assert!(previous.exists(), "the old directory is left alone");
    }

    #[test]
    fn nothing_happens_on_a_fresh_install() {
        let current = scratch_directory().join("id.itrium.honk");
        assert!(!adopt_previous(&current).unwrap());
        assert!(!current.exists());
    }
}
