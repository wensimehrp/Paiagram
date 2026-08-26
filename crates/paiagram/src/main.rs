//! The entrypoint of the application

use std::path::PathBuf;

use clap::Parser;
use paiagram::{App, UiState};
use serde::Deserialize;
use web_time::Instant;

struct PaiagramApp {
    app: App,
    ui_state: UiState,
    prev_time: Instant,
}

impl PaiagramApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        // egui_extras::install_image_loaders(&cc.egui_ctx);
        env_logger::init();
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
    todo!()
}
