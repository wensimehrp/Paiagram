use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Array;
use paiagram_oudia::{Root, parse_to_ast};
use wasm_bindgen::convert::FromWasmAbi;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, DragEvent, Event, EventTarget, File, HtmlAnchorElement, HtmlButtonElement,
    HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
};

const HIDE_THRESHOLD_BYTES: usize = 1_000;

#[derive(Clone, Copy)]
enum OutputFormat {
    AstDebug,
    Json,
    Yaml,
    Toml,
    Ron,
}

impl OutputFormat {
    fn from_value(value: &str) -> Self {
        match value {
            "json" => Self::Json,
            "yaml" => Self::Yaml,
            "toml" => Self::Toml,
            "ron" => Self::Ron,
            _ => Self::AstDebug,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::AstDebug => "txt",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Ron => "ron",
        }
    }
}

fn convert(input: &str, format: OutputFormat) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("Error: input is empty".into());
    }

    let ast = parse_to_ast(input).map_err(|err| format!("Error parsing oud/oud2 input:\n{err}"))?;

    if matches!(format, OutputFormat::AstDebug) {
        return Ok(format!("{ast:#?}"));
    }

    let ir = Root::try_from(ast.as_slice()).map_err(|err| format!("Error parsing AST:\n{err}"))?;

    let result = match format {
        OutputFormat::AstDebug => unreachable!(),
        OutputFormat::Json => serde_json::to_string_pretty(&ir).map_err(|e| e.to_string()),
        OutputFormat::Yaml => serde_yaml::to_string(&ir).map_err(|e| e.to_string()),
        OutputFormat::Toml => toml::to_string_pretty(&ir).map_err(|e| e.to_string()),
        OutputFormat::Ron => ron::ser::to_string_pretty(&ir, ron::ser::PrettyConfig::new())
            .map_err(|e| e.to_string()),
    };
    result.map_err(|err| {
        format!(
            "Error serializing IR to {}:\n{err}",
            format.extension().to_ascii_uppercase()
        )
    })
}

/// Converts the stored input text and writes the result into the output pane.
fn refresh_ui(
    input: &HtmlTextAreaElement,
    output: &HtmlTextAreaElement,
    select: &HtmlSelectElement,
    input_text: &RefCell<String>,
) {
    let text = input_text.borrow();
    let format = OutputFormat::from_value(&select.value());
    let converted = convert(&text, format).unwrap_or_else(|e| e);

    input.set_value(&text[..text.floor_char_boundary(HIDE_THRESHOLD_BYTES)]);
    output.set_value(&converted[..converted.floor_char_boundary(HIDE_THRESHOLD_BYTES)]);
}

fn load_file_into_input(
    file: File,
    input: &HtmlTextAreaElement,
    output: &HtmlTextAreaElement,
    select: &HtmlSelectElement,
    input_text: &Rc<RefCell<String>>,
) {
    let promise = file.text();
    wasm_bindgen_futures::spawn_local({
        let input = input.clone();
        let output = output.clone();
        let select = select.clone();
        let input_text = Rc::clone(input_text);
        async move {
            let Ok(js_value) = JsFuture::from(promise).await else {
                return;
            };
            let Some(text) = js_value.as_string() else {
                return;
            };
            input.set_value(&text);
            *input_text.borrow_mut() = text;
            refresh_ui(&input, &output, &select, &input_text);
        }
    });
}

/// Looks up an element by id (see `index.html` / `index.typ` for the ids).
fn element_by_id<E: JsCast>(document: &Document, id: &str) -> E {
    document
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("element #{id} not found"))
        .dyn_into::<E>()
        .unwrap_or_else(|_| panic!("element #{id} has an unexpected type"))
}

/// Adds an event listener and returns the closure that keeps it alive.
fn listen<E, F>(element: &EventTarget, event: &str, f: F) -> Closure<dyn FnMut(E)>
where
    E: FromWasmAbi + 'static,
    F: FnMut(E) + 'static,
{
    let closure = Closure::<dyn FnMut(E)>::wrap(Box::new(f) as Box<dyn FnMut(E)>);
    element
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .unwrap_or_else(|err| panic!("failed to add {event} listener: {err:?}"));
    closure
}

fn download_string(filename: &str, content: &str) -> Result<(), String> {
    let window = web_sys::window().expect("window is not available");
    let document = window.document().expect("document is not available");

    let blob = web_sys::Blob::new_with_str_sequence(&Array::of(&[JsValue::from_str(content)]))
        .map_err(|e| format!("Could not create blob: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("Could not create object URL: {e:?}"))?;

    let anchor = document
        .create_element("a")
        .map_err(|e| format!("Could not create anchor element: {e:?}"))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "Could not cast anchor element".to_string())?;

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

fn main() {
    let window = web_sys::window().expect("window is not available");
    let document = window.document().expect("document is not available");

    let input: HtmlTextAreaElement = element_by_id(&document, "input-textarea");
    let output: HtmlTextAreaElement = element_by_id(&document, "output-textarea");
    let select: HtmlSelectElement = element_by_id(&document, "output-format-select");
    let file_input: HtmlInputElement = element_by_id(&document, "file-upload");
    let copy_button: HtmlButtonElement = element_by_id(&document, "copy-output");
    let download_button: HtmlButtonElement = element_by_id(&document, "download-output");

    let input_text = Rc::new(RefCell::new(String::new()));

    let mut closures: Vec<Closure<dyn FnMut(Event)>> = Vec::new();

    closures.push(listen(&input, "input", {
        let input = input.clone();
        let output = output.clone();
        let select = select.clone();
        let input_text = input_text.clone();
        move |_event: Event| {
            *input_text.borrow_mut() = input.value();
            refresh_ui(&input, &output, &select, &input_text);
        }
    }));

    closures.push(listen(&select, "change", {
        let input = input.clone();
        let output = output.clone();
        let select = select.clone();
        let input_text = Rc::clone(&input_text);
        move |_event: Event| refresh_ui(&input, &output, &select, &input_text)
    }));

    closures.push(listen(&file_input, "change", {
        let input = input.clone();
        let output = output.clone();
        let select = select.clone();
        let file_input = file_input.clone();
        let input_text = Rc::clone(&input_text);
        move |_event: Event| {
            let Some(files) = file_input.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };
            load_file_into_input(file, &input, &output, &select, &input_text);
            // Allow picking the same file again later.
            file_input.set_value("");
        }
    }));

    closures.push(listen(&copy_button, "click", {
        let select = select.clone();
        let input_text = Rc::clone(&input_text);
        move |_event: Event| {
            let text = input_text.borrow();
            let window = web_sys::window().expect("window is not available");
            let contents = convert(&text, OutputFormat::from_value(&select.value()));
            drop(text);
            match contents {
                Ok(res) => {
                    let clipboard = window.navigator().clipboard();
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ = JsFuture::from(clipboard.write_text(&res)).await;
                    });
                    let _ = window.alert_with_message("Result copied");
                }
                Err(e) => {
                    let _ = window.alert_with_message(e.as_str());
                }
            }
        }
    }));

    closures.push(listen(&download_button, "click", {
        let select = select.clone();
        let input_text = Rc::clone(&input_text);
        move |_event: Event| {
            let text = input_text.borrow();
            let format = OutputFormat::from_value(&select.value());
            let filename = format!("converted-output.{}", format.extension());
            let contents = convert(&text, OutputFormat::from_value(&select.value()));
            drop(text);
            let contents = match contents {
                Ok(res) => res,
                Err(e) => {
                    let _ = window.alert_with_message(e.as_str());
                    return;
                }
            };
            if let Err(e) = download_string(&filename, &contents) {
                let _ = window.alert_with_message(&e);
            }
        }
    }));

    let mut drag_closures: Vec<Closure<dyn FnMut(DragEvent)>> = Vec::new();

    drag_closures.push(listen(&input, "dragover", |event: DragEvent| {
        event.prevent_default();
    }));

    drag_closures.push(listen(&input, "drop", {
        let input = input.clone();
        let output = output.clone();
        let select = select.clone();
        let input_text = Rc::clone(&input_text);
        move |event: DragEvent| {
            event.prevent_default();
            let Some(file) = (|| event.data_transfer()?.files()?.get(0))() else {
                return;
            };
            load_file_into_input(file, &input, &output, &select, &input_text);
        }
    }));

    // Initial render (empty input, default format from the <select>).
    refresh_ui(&input, &output, &select, &input_text);

    // The page is static, so the listeners live for the whole session.
    std::mem::forget(closures);
    std::mem::forget(drag_closures);
}
