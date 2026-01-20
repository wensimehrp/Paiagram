#[cfg(not(target_arch = "wasm32"))]
pub async fn write_file(data: Vec<u8>, filename: String) -> Result<(), std::io::Error> {
    let file = rfd::AsyncFileDialog::new()
        .set_file_name(&filename)
        .save_file()
        .await;
    if let Some(file) = file {
        file.write(&data).await?;
        bevy::log::info!("File saved to {:?}", file.path());
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function download_text_file(data, filename) {
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
    fn download_text_file(data: &[u8], filename: &str);
}

/// let the browser download a text file
#[cfg(target_arch = "wasm32")]
pub async fn write_file(data: Vec<u8>, filename: String) -> Result<(), String> {
    download_text_file(&data, &filename);
    Ok(())
}
