#[cfg(target_arch = "wasm32")]
mod paiagram_idb {
    pub const DATABASE: &str = "paiagram";
    pub const VERSION: u32 = 1;
}

#[cfg(target_arch = "wasm32")]
pub mod list_files {
    mod web;
    pub use web::*;
}

#[cfg(not(target_arch = "wasm32"))]
pub mod list_files {
    mod native;
    pub use native::*;
}

#[cfg(target_arch = "wasm32")]
pub mod save {
    pub const MAX_AUTOSAVES: usize = 10;
    mod web;
    pub use web::*;
}

#[cfg(not(target_arch = "wasm32"))]
pub mod save {
    pub const MAX_AUTOSAVES: usize = 10;
    mod native;
    pub use native::*;
}

use std::io::{self, Cursor, Seek, Write};
use std::sync::Arc;

use log::{info, warn};
use parking_lot::Mutex;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    self,
    js_sys::{Array, Uint8Array},
    wasm_bindgen::{JsCast, JsValue},
    {Blob, BlobPropertyBag, HtmlAnchorElement, Url},
};

/// The category to store the file in
#[derive(Clone, Copy, Debug)]
pub enum SaveCategory {
    /// A normal save file
    Save,
    /// An automatically crated save file
    Autosave,
}

/// The names of the saves available for loading.
#[derive(Clone, Debug, Default)]
pub struct AvailableSaves {
    pub saves: Vec<String>,
    pub autosaves: Vec<String>,
}

impl SaveCategory {
    /// Converts the category to a key namespace for IndexedDB.
    fn to_str(self) -> &'static str {
        match self {
            SaveCategory::Save => "normal_saves",
            SaveCategory::Autosave => "autosaves",
        }
    }
    /// Converts the category to a &Path for use in the filesystem
    #[cfg(not(target_arch = "wasm32"))]
    fn to_path(self) -> &'static std::path::Path {
        std::path::Path::new(self.to_str())
    }
}

#[derive(Default)]
pub enum FileWriteState {
    #[default]
    Idle,
    Processing,
    Done(io::Result<()>),
}

/// How to export some data to another format.
pub trait ExportObject: Sized + Send + 'static {
    /// Write content to a writer
    fn write_content<W: Write + Seek>(&mut self, writer: &mut W) -> io::Result<()>;
    /// The filename with extension.
    fn filename(&self) -> String;
    /// Export contents and save them on disk, with optional parameters
    fn write_to_file(mut self, state: Arc<Mutex<FileWriteState>>) {
        let filename = self.filename();
        *state.lock() = FileWriteState::Processing;

        // Serialize into memory. The actual file IO differs between native and wasm.
        let serialize = async move {
            let mut writer = Cursor::new(Vec::<u8>::new());
            let write_result = self.write_content(&mut writer);
            (write_result, writer.into_inner())
        };

        #[cfg(target_arch = "wasm32")]
        {
            let (tx, rx) = futures_channel::oneshot::channel::<io::Result<Vec<u8>>>();
            rayon::spawn(move || {
                let (write_result, buffer) = pollster::block_on(serialize);
                let _ = tx.send(write_result.map(|()| buffer));
            });
            wasm_bindgen_futures::spawn_local(async move {
                let result = match rx.await {
                    Ok(Ok(buffer)) => download_file(&filename, &buffer).map_err(|e| {
                        io::Error::other(format!("Error while downloading file: {e:?}"))
                    }),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(io::Error::other("serialization task was cancelled")),
                };
                *state.lock() = FileWriteState::Done(result);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        rayon::spawn(move || {
            let (write_result, buffer) = pollster::block_on(serialize);
            let result = match write_result {
                Ok(()) => {
                    let Some(path) = rfd::FileDialog::new().set_file_name(&filename).save_file()
                    else {
                        *state.lock() = FileWriteState::Idle;
                        return;
                    };
                    info!("Writing to file `{}`", path.display());
                    std::fs::write(path, buffer)
                }
                Err(e) => Err(e),
            };
            if let Err(ref e) = result {
                warn!("Failed to write `{}`: {}", filename, e);
            }
            *state.lock() = FileWriteState::Done(result);
        });
    }
}

/// Downloads the file in the browser.
///
/// This function must be run in the main thread. As a result, almost all file operations in
/// Paiagram must be completed on the main thread.
#[cfg(target_arch = "wasm32")]
fn download_file(filename: &str, content: &[u8]) -> Result<(), JsValue> {
    info!("Downloading `{}` ({} bytes)", filename, content.len());
    let anchor: HtmlAnchorElement = {
        let window = web_sys::window().expect("window is not available");
        let document = window.document().expect("document is not available");
        document.create_element("a").unwrap().dyn_into::<HtmlAnchorElement>().unwrap()
    };
    let blob = Blob::new_with_u8_array_sequence_and_options(
        &Array::of1(&Uint8Array::from(&content[..])),
        &{
            let bag = BlobPropertyBag::new();
            bag.set_type("application/octet-stream");
            bag
        },
    )?;

    let url = Url::create_object_url_with_blob(&blob)?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    if let Err(e) = Url::revoke_object_url(&url) {
        warn!("Failed to revoke object URL: {:?}", e);
    }
    Ok(())
}
