use bevy::{ecs::system::RunSystemOnce, log::LogPlugin, prelude::*};
#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;
use moonshine_core::{load::load_on_default_event, save::save_on_default_event};

mod colors;
mod graph;
mod i18n;
mod interface;
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
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(LogPlugin::default());
        app.add_plugins((
            interface::InterfacePlugin,
            graph::GraphPlugin,
            rw_data::RwDataPlugin,
            search::SearchPlugin,
            settings::SettingsPlugin,
            vehicles::VehiclesPlugin,
            lines::LinesPlugin,
            troubleshoot::TroubleShootPlugin,
        ))
        .add_observer(save_on_default_event)
        .add_observer(load_on_default_event);
        #[cfg(target_arch = "wasm32")]
        app.add_observer(rw_data::saveload::observe_autosave);
        info!("Initialized Bevy App.");
        // don't load autosave if opening a file or starting fresh
        let mut load_autosave = true;
        // get the world's settings resource to get the language
        let settings = app.world().resource::<settings::ApplicationSettings>();
        i18n::init(Some(settings.language.identifier()));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let args = Cli::parse();
            if args.open.is_some() || args.fresh {
                load_autosave = false;
            }
            if let Err(e) = app.world_mut().run_system_once_with(handle_args, args) {
                error!("Failed to handle command line arguments: {:?}", e);
            } else {
                info!("Command line arguments handled successfully.");
            }
        }
        if load_autosave
            && let Err(e) = app
                .world_mut()
                .run_system_once(rw_data::saveload::load_autosave)
        {
            error!("Failed to load autosave: {:?}", e);
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
        // this is true regardless of settings, as we always want to persist egui memory
        // autosave is handled separately
        true
    }
    fn auto_save_interval(&self) -> std::time::Duration {
        let mins = self
            .bevy_app
            .world()
            .resource::<settings::ApplicationSettings>()
            .autosave_interval_minutes;
        std::time::Duration::from_mins(mins as u64)
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // a dummy marker for the storage system
        // this saves stuff interval to egui e.g. window positions etc.
        eframe::set_value(storage, "autosave_marker", &());
        let autosave_enabled = self
            .bevy_app
            .world()
            .resource::<settings::ApplicationSettings>()
            .autosave_enabled;
        if !autosave_enabled {
            return;
        }
        // save the app state
        if let Err(e) = self
            .bevy_app
            .world_mut()
            .run_system_once(rw_data::saveload::autosave)
        {
            error!("Autosave failed: {:?}", e);
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[cfg(not(target_arch = "wasm32"))]
struct Cli {
    #[arg(
        short = 'o',
        long = "open",
        help = "Path to a .paiagram file (or any other compatible file formats) to open on startup"
    )]
    open: Option<String>,
    #[arg(
        long = "fresh",
        help = "Start with a fresh state, ignoring any autosave"
    )]
    fresh: bool,
    #[arg(
        long = "jgrpp",
        help = "Path to a set of OpenTTD JGRPP .json timetable export files.",
        num_args = 1..
    )]
    jgrpp_paths: Option<Vec<String>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_args(cli: In<Cli>, mut msg: MessageWriter<rw_data::ModifyData>, mut commands: Commands) {
    use rw_data::ModifyData;
    if let Some(path) = &cli.open {
        // match the ending of the path
        match path.split('.').next_back() {
            Some("paiagram") => {
                rw_data::saveload::load_save(&mut commands, path.into());
            }
            Some("json") | Some("pyetgr") => {
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
        return;
    }
    if let Some(paths) = &cli.jgrpp_paths {
        let mut contents = Vec::with_capacity(paths.len());
        for path in paths {
            contents.push(std::fs::read_to_string(path).expect("Failed to read file"));
        }
        msg.write(ModifyData::LoadJGRPP(contents));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
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
    i18n::init(None);
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
