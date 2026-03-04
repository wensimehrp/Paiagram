use bevy::tasks::IoTaskPool;
use num_format::{Locale, ToFormattedString};
use std::io::Write;

#[cfg(not(target_arch = "wasm32"))]
pub fn write_file<F>(filename: String, produce_data: F)
where
    F: FnOnce(&mut dyn Write) -> std::io::Result<()> + Send + 'static,
{
    IoTaskPool::get()
        .spawn(async move {
            let file = rfd::AsyncFileDialog::new()
                .set_file_name(&filename)
                .save_file()
                .await;
            let Some(file) = file else { return };
            let mut buf_writer = match std::fs::File::create(file.path()) {
                Ok(w) => std::io::BufWriter::new(w),
                Err(e) => {
                    bevy::log::error!(?e);
                    return;
                }
            };
            if let Err(e) = produce_data(&mut buf_writer) {
                bevy::log::error!(?e, "Failed while producing file contents");
                return;
            }
            if let Err(e) = buf_writer.flush() {
                bevy::log::error!(?e, "Failed to flush output");
                return;
            }
            bevy::log::info!("File saved to {:?}", file.path());
            let size = std::fs::metadata(file.path())
                .map(|m| m.len() as usize)
                .unwrap_or_default();
            bevy::log::info!("Filesize: {:?}", size.to_formatted_string(&Locale::en));
        })
        .detach();
}

// save_file() is not supported on wasm targets
// use browser download instead
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function download_file(data, filename) {
    const blob = new Blob([data], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
}
"#)]
extern "C" {
    fn download_file(data: &[u8], filename: &str);
}

/// let the browser download a text file
#[cfg(target_arch = "wasm32")]
pub fn write_file<F>(filename: String, produce_data: F)
where
    F: FnOnce(&mut dyn Write) -> std::io::Result<()> + 'static,
{
    IoTaskPool::get()
        .spawn(async move {
            let mut data = Vec::new();
            if let Err(e) = produce_data(&mut data) {
                bevy::log::error!(?e, "Failed while producing file contents");
                return;
            }
            download_file(&data, &filename);
            bevy::log::info!(
                "Filesize: {:?}",
                data.len().to_formatted_string(&Locale::en)
            );
        })
        .detach();
}
