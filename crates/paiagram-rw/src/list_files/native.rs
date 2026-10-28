use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::{AvailableSaves, SaveCategory};

pub type Result<T> = io::Result<T>;

/// Lists the saves of one category, newest last.
pub fn list_saves(base_dir: &Path, category: SaveCategory) -> io::Result<Vec<String>> {
    let dir = base_dir.join(category.to_path());
    let mut files = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("paia"))
            .filter_map(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    files.sort();
    Ok(files)
}

/// Lists the saves of both categories.
pub fn list_all_saves(tx: mpsc::Sender<io::Result<AvailableSaves>>, base_dir: PathBuf) {
    rayon::spawn(move || {
        let result = || {
            Ok(AvailableSaves {
                autosaves: list_saves(&base_dir, SaveCategory::Autosave)?,
                saves: list_saves(&base_dir, SaveCategory::Save)?,
            })
        };
        let _ = tx.send(result());
    });
}

/// Reads a save. If `key` is `None`, the newest save is loaded.
pub fn load_save(
    base_dir: &Path,
    category: SaveCategory,
    key: Option<&str>,
) -> io::Result<Option<Vec<u8>>> {
    let dir = base_dir.join(category.to_path());
    let path = match key {
        Some(key) => dir.join(format!("{key}.paia")),
        None => {
            let saves = list_saves(base_dir, category)?;
            let Some(key) = saves.last() else {
                return Ok(None);
            };
            dir.join(format!("{key}.paia"))
        }
    };

    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
