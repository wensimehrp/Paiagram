//! The entrypoint of the application

use std::path::PathBuf;

use clap::Parser;
use log::{error, info};
use paiagram::{App, UiState};
use serde::Deserialize;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use web_time::Instant;

struct PaiagramApp {
    app: App,
    ui_state: UiState,
    prev_time: Instant,
}

impl PaiagramApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        // egui_extras::install_image_loaders(&cc.egui_ctx);
        // set styles
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(2);
            style.interaction.selectable_labels = false;
        });
        Self {
            app: App::new(&cc.egui_ctx),
            ui_state: UiState::default(),
            prev_time: Instant::now(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_web_arguments() -> Arguments {
    if let Some(search) =
        eframe::web_sys::window().and_then(|window| window.location().search().ok())
    {
        info!("Handling web args... `{search}`");
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let curr_time = Instant::now();
        paiagram::show_ui(
            ui,
            &mut self.app,
            &mut self.ui_state,
            curr_time - self.prev_time,
        );
        self.prev_time = curr_time;
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
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Drawer")
            .with_app_id("Paiagram")
            .with_inner_size([1280.0, 720.0]),
        renderer: eframe::Renderer::Wgpu,
        multisampling: 4,
        ..Default::default()
    };
    let args = Arguments::parse();
    eframe::run_native(
        "Paiagram Drawer",
        native_options,
        Box::new(|cc| Ok(Box::new(PaiagramApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    use eframe::web_sys;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("No window");
        let document = window.document().expect("No document");
        let canvas = document
            .get_element_by_id("paiagram_canvas")
            .expect("Failed to find paiagram_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("paiagram_canvas was not a HtmlCanvasElement");

        info!("Starting application...");

        // initialize thread pool
        wasm_bindgen_rayon::init_thread_pool(window.navigator().hardware_concurrency() as usize)
            .await
            .unwrap();

        info!("Initialized thread pool");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(PaiagramApp::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(()) => {
                    loading_text.remove();
                }
                Err(err) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {err:?}");
                }
            }
        }
    });
}
