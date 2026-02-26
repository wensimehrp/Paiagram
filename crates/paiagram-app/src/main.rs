#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use bevy::{ecs::system::RunSystemOnce, log::LogPlugin, prelude::*};
#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;

use paiagram_core::{
    entry, graph, i18n, import, problems, route, rw, settings, station, trip, ui,
};
use paiagram_core::trip::class;

struct PaiagramApp {
    bevy_app: App,
}

impl PaiagramApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        cc.egui_ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(2);
            style.interaction.selectable_labels = false;
        });
        ui::apply_custom_fonts(&cc.egui_ctx);
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            cc.egui_ctx.data_mut(|data| {
                data.insert_temp(
                    egui::Id::new("wgpu_adapter_info"),
                    eframe::egui_wgpu::adapter_info_summary(&render_state.adapter.get_info()),
                );
                data.insert_temp(
                    egui::Id::new("wgpu_target_format"),
                    render_state.target_format,
                );
                let msaa_samples = if cfg!(target_arch = "wasm32") {
                    1_u32
                } else {
                    4_u32
                };
                data.insert_temp(egui::Id::new("wgpu_msaa_samples"), msaa_samples);
            });
        }
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(LogPlugin::default());
        app.add_plugins((
            ui::UiPlugin,
            entry::EntryPlugin,
            graph::GraphPlugin,
            route::RoutePlugin,
            import::ImportPlugin,
            trip::TripPlugin,
            station::StationPlugin,
            settings::SettingsPlugin,
            problems::ProblemsPlugin,
            class::ClassPlugin,
            rw::read::ReadPlugin,
            rw::save::SavePlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::new(2000),
        ));
        info!("Initialized Bevy App.");
        #[cfg(not(target_arch = "wasm32"))]
        {
            let args = Cli::parse();
            if let Err(e) = app.world_mut().run_system_once_with(handle_args, args) {
                error!("Failed to handle command line arguments: {:?}", e);
            } else {
                info!("Command line arguments handled successfully.");
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            info!("Handling web args...");
            if let Some(search) =
                eframe::web_sys::window().and_then(|it| it.location().search().ok())
            {
                info!(?search);
                let query = search.strip_prefix('?').unwrap_or(&search);
                for pair in query.split('&') {
                    let mut iter: std::str::SplitN<'_, char> = pair.splitn(2, '=');
                    let key = iter.next().unwrap_or_default();
                    if key != "load" {
                        continue;
                    }
                    let value = iter.next().unwrap_or_default();
                    app.world_mut()
                        .run_system_cached_with(handle_arg_pair, (key, value))
                        .unwrap();
                }
            }
        }
        Self { bevy_app: app }
    }
}

#[cfg(target_arch = "wasm32")]
fn handle_arg_pair((InRef(key), InRef(val)): (InRef<str>, InRef<str>), mut commands: Commands) {
    match key {
        "load" => {
            let Some(decoded) = urlencoding::decode(val).ok() else {
                return;
            };
            commands.trigger(import::DownloadFile {
                url: decoded.to_string(),
            });
        }
        key => {
            warn!("Unknown key in url: {}", key)
        }
    }
}

impl eframe::App for PaiagramApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.bevy_app.update();
        ui::show_ui(ctx, self.bevy_app.world_mut());
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[cfg(not(target_arch = "wasm32"))]
struct Cli {
    #[arg(
        short = 'o',
        long = "open",
        help = "Path to a .paiagram file (or any other compatible file formats) to open on startup",
        num_args = 1..
    )]
    open: Option<Vec<PathBuf>>,
    #[arg(
        long = "fresh",
        help = "Start with a fresh state, ignoring any autosave"
    )]
    fresh: bool,
    #[arg(
        long = "jgrpp",
        help = "Path to a set of OpenTTD JGRPP .json timetable export files. You may specify multiple by using the * syntax (usually this is expanded by your shell). Example: --jgrpp ~/.local/share/openttd/orderlist/*.json",
        num_args = 1..
    )]
    jgrpp_paths: Option<Vec<String>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_args(cli: In<Cli>, mut commands: Commands) {
    for path in cli.open.iter().flatten() {
        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(e) => {
                error!("Could not open {:?}: {:?}", path, e);
                continue;
            }
        };
        if let Err(e) = import::load_and_trigger(path, content, &mut commands) {
            error!("Could not load {:?}: {:#?}", path, e);
            continue;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    i18n::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Drawer")
            .with_app_id("Paiagram")
            .with_inner_size([1280.0, 720.0]),
        renderer: eframe::Renderer::Wgpu,
        multisampling: 4,
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
