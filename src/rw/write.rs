use bevy::tasks::IoTaskPool;

#[cfg(not(target_arch = "wasm32"))]
pub fn write_file(data: Vec<u8>, filename: String) {
    IoTaskPool::get()
        .spawn(async move {
            let file = rfd::AsyncFileDialog::new()
                .set_file_name(&filename)
                .save_file()
                .await;
            if let Some(file) = file {
                file.write(&data).await.unwrap();
                bevy::log::info!("File saved to {:?}", file.path());
            }
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
pub fn write_file(data: Vec<u8>, filename: String) {
    download_file(&data, &filename);
}
