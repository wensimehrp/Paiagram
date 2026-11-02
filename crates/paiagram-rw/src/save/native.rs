use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;

use anyhow::Context;
use log::{info, warn};

use crate::save::MAX_AUTOSAVES;
use crate::{ExportObject, SaveCategory, WriteResult};

impl SaveCategory {
    /// Saves `data` into this category's default storage.
    ///
    /// Normal saves use the object's default filename; autosaves and emergency saves are
    /// timestamped so they never overwrite one another. Only the latest [`MAX_AUTOSAVES`]
    /// autosaves are kept.
    pub fn save(self, mut data: impl ExportObject, tx: mpsc::Sender<WriteResult>) {
        rayon::spawn(move || {
            let result = (|| {
                let dir = self.category_path();
                fs::create_dir_all(&dir).context("Failed to create directory for save")?;
                let path = dir.join(format!("{}.{}", self.filename(), data.extension()));
                let buf = data
                    .write_to_vec(data.default_buf())
                    .context("Failed to serialize to buffer")?;
                let mut file = fs::File::create(&path).context("Failed to create file")?;
                file.write_all(&buf).context("Failed to write to file")?;
                file.sync_all().context("Failed to sync file with disk")?;
                if matches!(self, SaveCategory::Autosave) {
                    prune_autosaves(&dir);
                }
                Ok(path)
            })();
            let _ = tx.send(match result {
                Ok(path) => WriteResult::Success(path),
                Err(e) => WriteResult::Error(e),
            });
        });
    }
}

/// Keeps only the newest [`MAX_AUTOSAVES`] save files in `dir`.
fn prune_autosaves(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut autosaves: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("paia"))
        .collect();

    autosaves.sort();
    let excess = autosaves.len().saturating_sub(MAX_AUTOSAVES);
    for path in autosaves.into_iter().take(excess) {
        if let Err(error) = fs::remove_file(&path) {
            warn!(
                "Failed to remove old autosave `{}`: {error}",
                path.display()
            );
        }
    }
}
