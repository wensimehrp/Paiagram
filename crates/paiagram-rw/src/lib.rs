pub mod read;
pub mod save;
pub mod write;

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use log::{info, warn};

pub enum FileWriteState {
    Idle,
    Processing,
    Done(io::Result<()>),
}

/// How to export the current world to another format.
pub trait ExportObject: Sized + Send + 'static {
    /// Write content to a writer
    fn write_content<W: Write>(&mut self, writer: &mut W) -> io::Result<()>;
    /// the filename
    fn filename(&self) -> impl AsRef<str> {
        "exported_file"
    }
    fn extension(&self) -> impl AsRef<str>;
    /// Export contents and save them on disk, with optional parameters
    fn write_to_file<const COMPRESS: bool>(mut self, state: Arc<Mutex<FileWriteState>>) {
        let filename = {
            let mut filename = String::new();
            filename.push_str(self.filename().as_ref());
            filename.push_str(self.extension().as_ref());
            filename
        };
        rayon::spawn(move || {
            #[cfg(target_arch = "wasm32")]
            let mut buffer = Vec::new();
            #[cfg(not(target_arch = "wasm32"))]
            let mut buffer = {
                let Some(path) = rfd::FileDialog::new()
                    .set_file_name(filename)
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
            #[cfg(target_arch = "wasm32")]
            {
                *state.lock().unwrap_or_else(|e| e.into_inner()) =
                    FileWriteState::Done(download_file(&filename, &buffer).map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::Other,
                            format!("Error while downloading file: {:?}", e),
                        )
                    }));
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
use web_sys::wasm_bindgen::JsValue;

#[cfg(target_arch = "wasm32")]
fn download_file(filename: &str, content: &[u8]) -> Result<(), JsValue> {
    use web_sys::js_sys::{Array, Uint8Array};
    use web_sys::wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let window = web_sys::window().expect("window is not available");
    let document = window.document().expect("document is not available");

    let blob = Blob::new_with_u8_array_sequence_and_options(
        &Array::of1(&Uint8Array::from(&content[..])),
        &{
            let bag = BlobPropertyBag::new();
            bag.set_type("application/octet-stream");
            bag
        },
    )?;

    let url = Url::create_object_url_with_blob(&blob)?;
    let anchor = document.create_element("a")?.dyn_into::<HtmlAnchorElement>()?;

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();
    let _ = Url::revoke_object_url(&url);
    Ok(())
}
