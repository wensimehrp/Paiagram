use bevy::prelude::*;
mod interface;
mod intervals;
// mod lines;
mod rw_data;
mod search;
mod settings;
mod status_bar_text;
mod units;
mod vehicles;

use clap::Parser;

#[derive(Parser, Resource)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(
        short = 'o',
        long = "open",
        help = "Path to a .paiagram file (or any other compatible file formats) to open on startup"
    )]
    open: Option<String>,
}

/// Main entrypoint of the application.
fn main() {
    let args = Cli::parse();
    let app_window = Some(Window {
        title: "Paiagram Drawer".into(),
        fit_canvas_to_parent: true,
        ..default()
    });
    App::new()
        .insert_resource(args)
        .add_systems(Startup, handle_args)
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: app_window,
                ..default()
            }),
            bevy_framepace::FramepacePlugin,
            interface::InterfacePlugin,
            intervals::IntervalsPlugin,
            rw_data::RwDataPlugin,
            search::SearchPlugin,
            settings::SettingsPlugin,
            vehicles::VehiclesPlugin,
        ))
        .run();
}

fn handle_args(cli: Res<Cli>, mut msg: MessageWriter<rw_data::ModifyData>, mut commands: Commands) {
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
    commands.remove_resource::<Cli>();
}
