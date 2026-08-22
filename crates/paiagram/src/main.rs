//! The entrypoint of the application

use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use egui::Context;
#[cfg(not(target_arch = "wasm32"))]
use fontdb::Family;
use log::{info, warn};
use paiagram::{App, MainUiState};
use serde::Deserialize;

struct PaiagramApp {
    app: App,
    mus: MainUiState,
}

impl PaiagramApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        info!("Installed image loaders");
        // load translations
        env_logger::init();
        let en = include_str!("../assets/locales/en-CA.ftl");
        let zh = include_str!("../assets/locales/zh-Hans.ftl");
        egui_i18n::load_translations_from_text("en-CA", en).unwrap();
        egui_i18n::load_translations_from_text("zh-Hans", zh).unwrap();
        egui_i18n::set_language("en-CA");
        egui_i18n::set_fallback("en-CA");
        info!("Loaded translations");
        // set styles
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(2);
            style.interaction.selectable_labels = false;
        });
        load_font(&cc.egui_ctx);

        Self {
            app: App::new(),
            mus: MainUiState::default(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_font(ctx: &Context) {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let Some(face_id) = db.query(&fontdb::Query {
        families: &[Family::Name("Sarasa UI SC"), Family::SansSerif],
        ..Default::default()
    }) else {
        warn!("Couldn't select font from system");
        return;
    };
    let Some(bytes) = db.with_face_data(face_id, |font_bytes, _index| font_bytes.to_owned()) else {
        let font_name = db.face(face_id).map_or("<no name>", |info| &info.post_script_name);
        warn!("Couldn't load font named {:?}", font_name);
        return;
    };
    let bytes = Arc::new(bytes);
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "system_font".into(),
        Arc::new(egui::FontData::from_owned((*bytes).clone())),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "system_font".into());
    ctx.set_fonts(fonts);
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
