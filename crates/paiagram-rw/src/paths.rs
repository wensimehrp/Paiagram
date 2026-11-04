// SPDX-License-Identifier: MPL-2.0
//! Platform-specific save and storage directory resolution.

use std::path::PathBuf;

use directories::{BaseDirs, UserDirs};

const APP_DIR_NAME: &str = "paiagram";

/// Returns the base directory where Paiagram stores its save files.
///
/// Uses the user's documents directory (`XDG_DOCUMENTS_DIR` on Linux,
/// `~/Documents` on macOS, `{FOLDERID_Documents}` on Windows).
/// Falls back to the user's data directory if the documents directory is unavailable.
pub fn storage_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(doc_dir) =
        std::env::var_os("XDG_DOCUMENTS_DIR").map(PathBuf::from).filter(|p| p.is_absolute())
    {
        return Some(doc_dir.join(APP_DIR_NAME));
    }

    if let Some(user_dirs) = UserDirs::new() {
        if let Some(doc_dir) = user_dirs.document_dir() {
            return Some(doc_dir.to_path_buf().join(APP_DIR_NAME));
        }
        return Some(user_dirs.home_dir().join("Documents").join(APP_DIR_NAME));
    }

    BaseDirs::new().map(|base_dirs| base_dirs.data_dir().join(APP_DIR_NAME))
}
