//! The entrypoint of the application

use std::path::PathBuf;

use clap::Parser;
use paiagram::{App, MainUiState};
use serde::Deserialize;

struct PaiagramApp {
    app: App,
    mus: MainUiState,
}

impl PaiagramApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(2);
            style.interaction.selectable_labels = false;
        });
        Self {
            app: App::new(),
            mus: MainUiState::default(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_web_arguments() -> Arguments {
    if let Some(search) =
        eframe::web_sys::window().and_then(|window| window.location().search().ok())
    {
        info!(?search, "Handling web args...");
        let query = search.strip_prefix('?').unwrap_or(&search);
        match serde_html_form::from_str::<Arguments>(&query) {
            Ok(args) => return args,
            Err(error) => error!("Failed to parse web args: {error}"),
        }
    }

    Arguments::default()
}

impl eframe::App for PaiagramApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        paiagram::show_ui(ui, &mut self.app, &mut self.mus);
    }
}

/// Arguments for the application.
#[derive(Parser, Default, Deserialize)]
#[command(version, about, long_about = None)]
struct Arguments {
    #[arg(
        short = 'o',
        long = "open",
        help = "Path to a .paiagram file (or any other compatible file formats) to open on startup",
        num_args = 1..
    )]
    #[serde(default)]
    open: Option<Vec<PathBuf>>,
    #[arg(long = "locale", help = "Set the localization")]
    #[serde(default)]
    locale: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Drawer")
            .with_app_id("Paiagram")
            .with_inner_size([1280.0, 720.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            desired_maximum_frame_latency: Some(2),
            ..Default::default()
        },
        multisampling: 4,
        ..Default::default()
    };
    eframe::run_native(
        "Paiagram Drawer",
        native_options,
        Box::new(|cc| Ok(Box::new(PaiagramApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
fn main() {
    i18n::init();
    use eframe::wasm_bindgen::JsCast as _;
    use eframe::web_sys;

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = if let Some(canvas) = document.get_element_by_id("paiagram_canvas") {
            canvas
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("paiagram_canvas was not a HtmlCanvasElement")
        } else {
            let canvas = document
                .create_element("canvas")
                .expect("Failed to create canvas element");
            canvas.set_id("paiagram_canvas");

            canvas
                .set_attribute("style", "display: block; width: 100%; height: 100%;")
                .ok();

            let body = document.body().expect("Failed to get document body");
            body.set_attribute(
                "style",
                "margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden;",
            )
            .ok();

            let html = document.document_element().expect("No document element");
            html.set_attribute(
                "style",
                "margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden;",
            )
            .ok();

            body.append_child(&canvas).expect("Failed to append canvas");
            canvas
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("Failed to cast canvas")
        };

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(PaiagramApp::new(cc)))),
            )
            .await;

        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
