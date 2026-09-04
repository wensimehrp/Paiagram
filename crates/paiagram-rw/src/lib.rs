pub mod read;
pub mod save;
pub mod write;

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use log::{info, warn};
#[cfg(target_arch = "wasm32")]
use web_sys::js_sys::{Array, Uint8Array};
#[cfg(target_arch = "wasm32")]
use web_sys::wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

pub enum FileWriteState {
    Idle,
    Processing,
    Done(io::Result<()>),
}

/// How to export some data to another format.
pub trait ExportObject: Sized + Send + 'static {
    /// Write content to a writer
    fn write_content<W: Write>(&mut self, writer: &mut W) -> io::Result<()>;
    /// The filename.
    fn filename(&self) -> impl AsRef<str> {
        "exported_file"
    }
    /// The extension including the proceding dot.
    fn extension(&self) -> impl AsRef<str>;
    /// Export contents and save them on disk, with optional parameters
    fn write_to_file<const COMPRESS: bool>(mut self, state: Arc<Mutex<FileWriteState>>) {
        let filename = {
            let mut filename = String::new();
            filename.push_str(self.filename().as_ref());
            filename.push_str(self.extension().as_ref());
            filename
        };
        #[cfg(target_arch = "wasm32")]
        {
            let (tx, rx) = futures_channel::oneshot::channel::<io::Result<Vec<u8>>>();

            info!("Preparing to download `{}`", filename);
            *state.lock().unwrap_or_else(|e| e.into_inner()) = FileWriteState::Processing;

            // Serialize on the rayon thread pool and send the buffer back over a
            // oneshot channel, so the DOM is only touched here on the main thread.
            rayon::spawn(move || {
                let mut buffer = Vec::new();
                let res = if COMPRESS {
                    let mut writer = zrip::FrameEncoder::new(&mut buffer, 4).unwrap();
                    let r = self.write_content(&mut writer);
                    writer.finish().unwrap();
                    r
                } else {
                    self.write_content(&mut buffer)
                };
                let _ = tx.send(res.map(|_| buffer));
            });

            wasm_bindgen_futures::spawn_local(async move {
                match rx.await {
                    Ok(Ok(buffer)) => {
                        let res = download_file(&filename, &buffer).map_err(|e| {
                            io::Error::other(format!("Error while downloading file: {:?}", e))
                        });
                        *state.lock().unwrap_or_else(|e| e.into_inner()) =
                            FileWriteState::Done(res);
                    }
                    Ok(Err(e)) => {
                        warn!("Failed to serialize: {}", e);
                        *state.lock().unwrap_or_else(|e| e.into_inner()) =
                            FileWriteState::Done(io::Result::Err(e));
                    }
                    Err(_) => {
                        warn!("Serialization task was dropped before sending a result");
                        *state.lock().unwrap_or_else(|e| e.into_inner()) = FileWriteState::Done(
                            Err(io::Error::other("serialization task was cancelled")),
                        );
                    }
                }
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            rayon::spawn(move || {
                let mut buffer = {
                    let Some(path) = rfd::FileDialog::new()
                        .set_file_name(filename.as_str())
                        .add_filter("Exported", &[self.extension().as_ref()])
                        .save_file()
                    else {
                        return;
                    };
                    info!("Writing to file `{}`", path.display());
                    match std::fs::File::create(path) {
                        Ok(path) => path,
                        Err(e) => {
                            warn!("Failed to open file: {}", e);
                            return;
                        }
                    }
                };
                *state.lock().unwrap_or_else(|e| e.into_inner()) = FileWriteState::Processing;
                let res = if COMPRESS {
                    let mut writer = zrip::FrameEncoder::new(&mut buffer, 4).unwrap();
                    let r = self.write_content(&mut writer);
                    writer.finish().unwrap();
                    r
                } else {
                    self.write_content(&mut buffer)
                };
                if let Err(e) = res {
                    warn!("Failed to serialize: {}", e);
                    *state.lock().unwrap_or_else(|e| e.into_inner()) =
                        FileWriteState::Done(io::Result::Err(e));
                    return;
                }
                info!("Successfully wrote `{}`", filename);
                *state.lock().unwrap_or_else(|e| e.into_inner()) = FileWriteState::Done(Ok(()));
            });
        }
    }
}

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
