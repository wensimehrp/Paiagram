use std::io;
use std::sync::mpsc;

use crate::{AvailableSaves, SaveCategory};

pub type Result<T> = io::Result<T>;

impl SaveCategory {
    /// Lists the saves of this category, newest last.
    pub fn list_files(self) -> io::Result<Vec<String>> {
        let dir = self.category_path();
        let entries = std::fs::read_dir(dir)?;
        let mut files = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("paia"))
            .filter_map(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
            .collect::<Vec<_>>();
        files.sort();
        Ok(files)
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
