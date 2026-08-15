use js_sys::Array;
use leptos::prelude::*;
use std::cmp::min;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{DragEvent, Event, File, HtmlAnchorElement, HtmlInputElement};

use paiagram_oudia::{Root, parse_to_ast};

const HIDE_THRESHOLD_CHARS: usize = 20_000;
const PREVIEW_CHARS: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    AstDebug,
    Json,
    Yaml,
    Toml,
}

impl OutputFormat {
    fn from_value(value: &str) -> Self {
        match value {
            "json" => Self::Json,
            "yaml" => Self::Yaml,
            "toml" => Self::Toml,
            _ => Self::AstDebug,
        }
    }

    fn as_value(self) -> &'static str {
        match self {
            Self::AstDebug => "ast",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::AstDebug => "txt",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn preview_text(text: &str) -> String {
    let total = char_len(text);
    if total <= HIDE_THRESHOLD_CHARS {
        return text.to_string();
    }

    let visible_count = min(PREVIEW_CHARS, total);
    let shown: String = text.chars().take(visible_count).collect();
    let hidden = total.saturating_sub(visible_count);

    format!("{shown}\n\n[... hidden {hidden} characters ...]")
}

fn convert(input: &str, format: OutputFormat) -> String {
    if input.trim().is_empty() {
        return String::new();
    }

    let ast = match parse_to_ast(input) {
        Ok(ast) => ast,
        Err(err) => return format!("Error parsing oud/oud2 input:\n{err}"),
    };

    match format {
        OutputFormat::AstDebug => format!("{ast:#?}"),
        OutputFormat::Json => {
            let ir = match Root::try_from(ast.as_slice()) {
                Ok(ir) => ir,
                Err(err) => return format!("Error converting AST to IR:\n{err}"),
            };
            match serde_json::to_string_pretty(&ir) {
                Ok(output) => output,
                Err(err) => format!("Error serializing IR to JSON:\n{err}"),
            }
        }
        OutputFormat::Yaml => {
            let ir = match Root::try_from(ast.as_slice()) {
                Ok(ir) => ir,
                Err(err) => return format!("Error converting AST to IR:\n{err}"),
            };
            match serde_yaml::to_string(&ir) {
                Ok(output) => output,
                Err(err) => format!("Error serializing IR to YAML:\n{err}"),
            }
        }
        OutputFormat::Toml => {
            let ir = match Root::try_from(ast.as_slice()) {
                Ok(ir) => ir,
                Err(err) => return format!("Error converting AST to IR:\n{err}"),
            };
            match toml::to_string_pretty(&ir) {
                Ok(output) => output,
                Err(err) => format!("Error serializing IR to TOML:\n{err}"),
            }
        }
    }
}

fn load_file_into_input(file: File, set_input: WriteSignal<String>) {
    let promise = file.text();

    leptos::task::spawn_local(async move {
        if let Ok(js_value) = JsFuture::from(promise).await {
            if let Some(text) = js_value.as_string() {
                set_input.set(text);
            }
        }
    });
}

fn download_text_file(filename: &str, contents: &str) -> Result<(), String> {
    let Some(window) = web_sys::window() else {
        return Err("Window is not available".to_string());
    };
    let Some(document) = window.document() else {
        return Err("Document is not available".to_string());
    };

    let parts = Array::new();
    parts.push(&JsValue::from_str(contents));
    let blob = web_sys::Blob::new_with_str_sequence(&parts)
        .map_err(|e| format!("Could not create blob: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("Could not create object URL: {e:?}"))?;

    let anchor = document
        .create_element("a")
        .map_err(|e| format!("Could not create anchor element: {e:?}"))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "Could not cast anchor element".to_string())?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

#[component]
fn App() -> impl IntoView {
    let (input, set_input) = signal(String::new());
    let (format, set_format) = signal(OutputFormat::AstDebug);

    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    let output = Memo::new(move |_| convert(&input.get(), format.get()));

    let input_too_long = Memo::new(move |_| char_len(&input.get()) > HIDE_THRESHOLD_CHARS);
    let output_too_long = Memo::new(move |_| char_len(&output.get()) > HIDE_THRESHOLD_CHARS);

    let visible_input = Memo::new(move |_| preview_text(&input.get()));
    let visible_output = Memo::new(move |_| preview_text(&output.get()));
    let has_conversion_error = Memo::new(move |_| output.get().starts_with("Error "));

    let open_file = {
        let file_input_ref = file_input_ref.clone();
        move |_| {
            if let Some(input_element) = file_input_ref.get() {
                input_element.set_value("");
                input_element.click();
            }
        }
    };

    let on_file_input_change = move |ev: Event| {
        let Some(target) = ev
            .target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };

        let Some(files) = target.files() else {
            return;
        };

        let Some(file) = files.get(0) else {
            return;
        };

        load_file_into_input(file, set_input);
    };

    let on_drop = move |ev: DragEvent| {
        ev.prevent_default();

        let Some(data_transfer) = ev.data_transfer() else {
            return;
        };

        let Some(files) = data_transfer.files() else {
            return;
        };

        let Some(file) = files.get(0) else {
            return;
        };

        load_file_into_input(file, set_input);
    };

    let copy_output = move |_| {
        let contents = output.get_untracked();
        if contents.is_empty() {
            return;
        }

        let Some(window) = web_sys::window() else {
            return;
        };

        let clipboard = window.navigator().clipboard();

        leptos::task::spawn_local(async move {
            let _ = JsFuture::from(clipboard.write_text(&contents)).await;
        });
    };

    let download_output = move |_| {
        let contents = output.get_untracked();
        if contents.is_empty() {
            return;
        }

        let ext = format.get_untracked().extension();
        let filename = format!("converted-output.{ext}");
        let _ = download_text_file(&filename, &contents);
    };

    view! {
        <main class="app">
            <input
                node_ref=file_input_ref
                class="visually-hidden"
                type="file"
                accept=".oud,.oud2"
                on:change=on_file_input_change
            />

            <h1 class="app__title">
                <a
                    class="app__title-link"
                    href="https://github.com/WenSimEHRP/Paiagram-oudia"
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    "OuDia(Second) Converter"
                </a>
            </h1>

            <div class="toolbar">
                <label class="toolbar__field">
                    <span>"Output format"</span>
                    <select
                        on:change=move |ev| set_format.set(OutputFormat::from_value(&event_target_value(&ev)))
                        prop:value=move || format.get().as_value()
                    >
                        <option value="ast">"AST debug print"</option>
                        <option value="json">"JSON (IR)"</option>
                        <option value="yaml">"YAML (IR)"</option>
                        <option value="toml">"TOML (IR)"</option>
                    </select>
                </label>

                <button type="button" on:click=open_file>"Open file"</button>
                <button type="button" on:click=copy_output>"Copy output"</button>
                <button type="button" on:click=download_output>"Download output"</button>
            </div>

            <section class="panes">
                <div class="pane">
                    <div class="pane__header">
                        <span>"Input (raw oud/oud2 text)"</span>
                        <Show when=move || input_too_long.get() fallback=|| ()>
                            <span class="pane__hint">"Long input hidden"</span>
                        </Show>
                    </div>

                    <textarea
                        class="pane__textarea"
                        class:drop-target=move || input_too_long.get()
                        placeholder="Paste raw .oud/.oud2 content here, or drag a file here..."
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                        on:dragover=move |ev| ev.prevent_default()
                        on:drop=on_drop
                        prop:value=move || visible_input.get()
                        prop:readonly=move || input_too_long.get()
                    />
                </div>

                <div class="pane">
                    <div class="pane__header">
                        <span>"Output"</span>
                        <Show when=move || output_too_long.get() fallback=|| ()>
                            <span class="pane__hint">"Long output hidden"</span>
                        </Show>
                    </div>

                    <textarea
                        class="pane__textarea output-readonly"
                        class:error-state=move || has_conversion_error.get()
                        readonly
                        prop:value=move || visible_output.get()
                    />
                </div>
            </section>

            <footer class="footer">
                <span>"© Jeremy Gao"</span>
                <a href="https://github.com/WenSimEHRP/Paiagram-oudia" target="_blank" rel="noopener noreferrer">
                    "GitHub"
                </a>
            </footer>
        </main>
    }
}

fn main() {
    mount_to_body(App);
}
