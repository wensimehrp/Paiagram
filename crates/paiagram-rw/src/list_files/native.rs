use std::sync::mpsc;
use std::{fs, io};

use crate::{AvailableSaves, SaveCategory};

pub type Result<T> = io::Result<T>;

impl SaveCategory {
    /// Lists the saves of this category, newest last.
    pub fn list_files(&self) -> io::Result<Vec<String>> {
        let dir = self.category_path();
        let Ok(entries) = fs::read_dir(dir) else {
            return Ok(Vec::new());
        };
        let mut files = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("paia"))
            .filter_map(|path| path.into_string().ok())
            .collect::<Vec<_>>();
        files.sort();
        Ok(files)
    }
    /// Load a file given the filename
    pub fn load_file(&self, filename: &str) -> io::Result<Vec<u8>> {
        let dir = self.category_path();
        fs::read(dir.join(filename))
    }
}

/// Lists the saves of both categories.
pub fn list_all_saves(tx: mpsc::Sender<io::Result<AvailableSaves>>) {
    rayon::spawn(move || {
        let result = || {
            Ok(AvailableSaves {
                autosaves: SaveCategory::Autosave.list_files()?,
                saves: SaveCategory::Save(None).list_files()?,
            })
        };
        let _ = tx.send(result());
    });
}
