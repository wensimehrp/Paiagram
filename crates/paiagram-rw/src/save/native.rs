use log::{info, warn};

use crate::SaveCategory;
use crate::save::MAX_AUTOSAVES;

/// Save an [`ExportObject`] into IndexedDB.
/// Typically used to store an app save.
///
/// This function keeps the latest [`MAX_AUTOSAVES`] files.
pub fn save_to_dir(
    base_dir: impl AsRef<std::path::Path>,
    category: SaveCategory,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let dir = base_dir.as_ref().join(category.to_path());

    std::fs::create_dir_all(&dir)?;
    let timestamp = jiff::Timestamp::now();
    let filename = format!("{}.paia", timestamp.strftime("%Y%m%dT%H%M%S%3fZ"));
    let path = dir.join(filename.clone());
    let mut file = std::fs::File::create(&path)?;
    if let Err(error) = write(&mut file) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    file.sync_all()?;

    info!("Autosaved {}", path.display());

    if !matches!(category, SaveCategory::Autosave) {
        return Ok(());
    }

    // Timestamped file names sort chronologically, so the first entries are the oldest.
    let mut autosaves: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("paia"))
        .collect();
    autosaves.sort();
    let excess = autosaves.len().saturating_sub(MAX_AUTOSAVES);
    for path in autosaves.into_iter().take(excess) {
        if let Err(error) = std::fs::remove_file(&path) {
            warn!(
                "Failed to remove old autosave `{}`: {error}",
                path.display()
            );
        }
    }
    Ok(())
}
