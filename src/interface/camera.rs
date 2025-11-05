use bevy::color::palettes::tailwind;
use bevy::prelude::*;

pub fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Name::new("Egui Camera"),
        bevy_camera::visibility::RenderLayers::none(),
        bevy_egui::PrimaryEguiContext,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::Srgba(tailwind::GRAY_900)),
            ..default()
        },
    ));
}
