use bevy::{ecs::system::RunSystemOnce, log::LogPlugin, prelude::*};
use clap::Parser;

mod colors;
mod i18n;
mod interface;
mod intervals;
mod lines;
mod rw_data;
mod search;
mod settings;
mod status_bar_text;
mod troubleshoot;
mod units;
mod vehicles;

struct PaiagramApp {
    bevy_app: App,
}

impl PaiagramApp {
    fn new(_cc: &eframe::CreationContext) -> Self {
        // TODO: handle load from storage
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(LogPlugin::default());
        app.add_plugins((
            interface::InterfacePlugin,
            intervals::IntervalsPlugin,
            rw_data::RwDataPlugin,
            search::SearchPlugin,
            settings::SettingsPlugin,
            vehicles::VehiclesPlugin,
            lines::LinesPlugin,
            troubleshoot::TroubleShootPlugin,
        ));
        let args = Cli::parse();
        if let Err(e) = app.world_mut().run_system_once_with(handle_args, args) {
            error!("Failed to handle command line arguments: {:?}", e);
        }
        Self { bevy_app: app }
    }
}

impl eframe::App for PaiagramApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.bevy_app
            .world_mut()
            .resource_mut::<interface::MiscUiState>()
            .on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);
        self.bevy_app.update();
        if let Err(e) = interface::show_ui(self, ctx) {
            error!("UI Error: {:?}", e);
        }
    }
    fn persist_egui_memory(&self) -> bool {
        true
    }
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_mins(5)
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Err(e) = rw_data::save::autosave(&mut self.bevy_app, storage) {
            error!("Autosave failed: {:?}", e);
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(
        short = 'o',
        long = "open",
        help = "Path to a .paiagram file (or any other compatible file formats) to open on startup"
    )]
open: Option<String>,
}

fn handle_args(cli: In<Cli>, mut msg: MessageWriter<rw_data::ModifyData>) {
    if let Some(path) = &cli.open {
        use rw_data::ModifyData;
        // match the ending of the path
        match path.split('.').next_back() {
            Some("paiagram") => {
                warn!("Opening .paiagram files is not yet implemented.");
            }
            Some("json") | Some("pyetgr") => {
                // read the file
                let file_content = std::fs::read_to_string(path).expect("Failed to read file");
                msg.write(ModifyData::LoadQETRC(file_content));
            }
            Some("oud2") => {
                let file_content = std::fs::read_to_string(path).expect("Failed to read file");
                msg.write(ModifyData::LoadOuDiaSecond(file_content));
            }
            _ => {
                warn!("Unsupported file format: {}", path);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    i18n::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Paiagram Drawer")
            .with_inner_size([1280.0, 720.0]),
        ..default()
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

// When compiling to web using trunk:
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

            // Set styles to ensure full screen and correct rendering
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

        // Remove the loading text and spinner:
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
