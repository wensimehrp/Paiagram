#[cfg(not(target_arch = "wasm32"))]
pub fn write_text_file(data: &str, filename: &str) -> Result<(), std::io::Error> {
    let mut save_dialog = rfd::FileDialog::new();
    save_dialog = save_dialog.set_file_name(filename);
    if let Some(path) = save_dialog.save_file() {
        std::fs::write(&path, data)?;
        bevy::log::info!("File saved to {:?}", path);
    }
    Ok(())
}

/// let the browser download a text file
#[cfg(target_arch = "wasm32")]
pub fn write_text_file(data: &str, filename: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, Url};

    let window = web_sys::window().ok_or("no global window found")?;
    let document = window.document().ok_or("should have a document on window")?;
    let body = document.body().ok_or("document should have a body")?;

    // 1. Create a Blob from the string data
    let mut properties = BlobPropertyBag::new();
    properties.type_("text/plain");

    let data_array = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(data));
    let blob = Blob::new_with_str_sequence_and_options(&data_array, &properties)
        .map_err(|e| format!("Failed to create blob: {:?}", e))?;

    // 2. Create a temporary URL for the blob
    let url = Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("Failed to create URL: {:?}", e))?;

    // 3. Create a hidden <a> element
    let anchor = document
        .create_element("a")
        .map_err(|e| format!("Failed to create anchor: {:?}", e))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|e| format!("Failed to cast to anchor: {:?}", e))?;

    anchor.set_href(&url);
    anchor.set_download(filename);

    // 4. Append, click, and remove the anchor
    body.append_child(&anchor)
        .map_err(|e| format!("Failed to append anchor: {:?}", e))?;
    anchor.click();
    body.remove_child(&anchor)
        .map_err(|e| format!("Failed to remove anchor: {:?}", e))?;

    // 5. Clean up the URL
    Url::revoke_object_url(&url).map_err(|e| format!("Failed to revoke URL: {:?}", e))?;

    Ok(())
}
