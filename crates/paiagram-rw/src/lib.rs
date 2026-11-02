// SPDX-License-Identifier: MPL-2.0
//! Requirements:
//!
//! Linux - always enbale XDG

use std::borrow::Cow;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::Context;

#[cfg(target_arch = "wasm32")]
mod paiagram_idb {
    pub const DATABASE: &str = "paiagram";
    pub const VERSION: u32 = 1;
}

pub mod list_files {
    #[cfg(not(target_arch = "wasm32"))]
    mod native;
    #[cfg(not(target_arch = "wasm32"))]
    pub use native::*;
    #[cfg(target_arch = "wasm32")]
    mod web;
    #[cfg(target_arch = "wasm32")]
    pub use web::*;
}

pub mod save {
    pub const MAX_AUTOSAVES: usize = 10;
    #[cfg(not(target_arch = "wasm32"))]
    mod native;
    #[cfg(not(target_arch = "wasm32"))]
    pub use native::*;
    #[cfg(target_arch = "wasm32")]
    mod web;
    #[cfg(target_arch = "wasm32")]
    pub use web::*;
}

/// The category to store the file in
#[derive(Clone, Debug)]
pub enum SaveCategory {
    /// A normal save file, with an optional filename
    Save(Option<String>),
    /// An automatically crated save file
    Autosave,
    /// Created on emergency cases, such as a crash or a dump
    Emergency,
}

/// The names of the saves available for loading.
#[derive(Clone, Debug, Default)]
pub struct AvailableSaves {
    pub saves: Vec<String>,
    pub autosaves: Vec<String>,
}

impl SaveCategory {
    /// Converts the category to a key namespace for IndexedDB, or provide the subdirectory's name.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        deprecated(
            note = "You usually won't use category_str() directly. Use category_path() instead"
        )
    )]
    fn category_str(&self) -> &'static str {
        match self {
            SaveCategory::Save(_) => "normal_saves",
            SaveCategory::Autosave => "autosaves",
            SaveCategory::Emergency => "emergency",
        }
    }
    /// The filename, if using this category
    fn filename(&self) -> Cow<'_, str> {
        if let SaveCategory::Save(Some(name)) = self {
            return name.into();
        }
        let timestamp = jiff::Timestamp::now();
        timestamp.to_string().into()
    }
    /// The base storage directory on native platform. Uses [`eframe::storage_dir`] by default
    #[cfg(not(target_arch = "wasm32"))]
    fn base_storage_dir() -> PathBuf {
        eframe::storage_dir("paiagram").expect("eframe should provide a storage dir")
    }
    /// The path to the category on native platform.
    #[cfg(not(target_arch = "wasm32"))]
    fn category_path(&self) -> PathBuf {
        Self::base_storage_dir().join(self.category_str())
    }
}

/// Results to writing a file
pub enum WriteResult {
    /// The write failed with an error message
    Error(anyhow::Error),
    /// The write successed with the returned path
    Success(PathBuf),
    /// The write was cancelled
    Aborted,
}

/// How to export some data to another format.
pub trait ExportObject: Sized + Send + 'static {
    /// Write the content into `buf`, returning the (possibly grown) buffer.
    fn write_to_vec(&mut self, buf: Vec<u8>) -> io::Result<Vec<u8>>;
    /// The extension of the file without the dot.
    fn extension(&self) -> &str;
    /// The default filename without the extension.
    fn default_filename(&self) -> &str {
        "exported_data"
    }
    /// The default buffer size. The default size is 10 MiB.
    fn default_buf(&self) -> Vec<u8> {
        Vec::with_capacity(10 * 1024 * 1024)
    }
    /// Serialize the content and let the user pick where to save it.
    #[cfg(not(target_arch = "wasm32"))]
    fn write_via_interface(mut self, tx: mpsc::Sender<WriteResult>) {
        let filename = format!("{}.{}", self.default_filename(), self.extension());
        rayon::spawn(move || {
            let result: anyhow::Result<_> = (|| {
                let buf = self
                    .write_to_vec(self.default_buf())
                    .context("Failed to serialize content to buffer.")?;
                let Some(path) = rfd::FileDialog::new().set_file_name(&filename).save_file() else {
                    return Ok(None);
                };
                std::fs::write(path.clone(), buf)
                    .with_context(|| format!("Failed to write to path `{}`.", path.display()))?;
                Ok(Some(path))
            })();
            let _ = tx.send(match result {
                Ok(Some(path)) => WriteResult::Success(path),
                Ok(None) => WriteResult::Aborted,
                Err(e) => WriteResult::Error(e),
            });
        });
    }
    /// Serialize the content and download it in the browser.
    #[cfg(target_arch = "wasm32")]
    fn write_via_interface(mut self, tx: mpsc::Sender<WriteResult>) {
        let filename = format!("{}.{}", self.default_filename(), self.extension());
        let (inner_tx, inner_rx) = futures_channel::oneshot::channel::<io::Result<Vec<u8>>>();
        rayon::spawn(move || {
            let _ = inner_tx.send(self.write_to_vec(self.default_buf()));
        });
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let buf = inner_rx
                    .await
                    .context("Failed to receive serialized data.")?
                    .context("Failed to serialize content to buffer.")?;
                crate::save::download_file(&filename, &buf)
                    .map(|_| PathBuf::from(filename))
                    .map_err(|_| anyhow::anyhow!("Failed to download file"))
            };
            let _ = tx.send(match result.await {
                Ok(path) => WriteResult::Success(path),
                Err(e) => WriteResult::Error(e),
            });
        });
    }
}

// write =========================================

pub enum LoadResult<T> {
    Success(T),
    Aborted,
    Failed(anyhow::Error),
}
