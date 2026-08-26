use std::sync::{Arc, LazyLock};

use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use fontdb::{Family, ID};
use log::{info, warn};

pub(crate) static FONT_DATABASE: LazyLock<fontdb::Database> = LazyLock::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    db
});

pub(crate) fn load_default_font(ctx: &egui::Context) -> &str {
    // dynamic query
    let Some(face_id) = FONT_DATABASE.query(&fontdb::Query {
        families: &[Family::Name("Sarasa UI SC"), Family::SansSerif],
        ..Default::default()
    }) else {
        warn!("Couldn't select font from system");
        return "Error";
    };
    let font_name = FONT_DATABASE.face(face_id).map_or("<no name>", |info| &info.post_script_name);
    info!("Trying to load font `{}`", font_name);
    load_font_to_egui(face_id, ctx, FontDefinitions::default());
    font_name
}

pub(crate) fn load_font_to_egui(
    face_id: ID,
    ctx: &egui::Context,
    mut definitions: FontDefinitions,
) {
    let Some(bytes) =
        FONT_DATABASE.with_face_data(face_id, |font_bytes, _index| font_bytes.to_owned())
    else {
        warn!("Couldn't load font!");
        return;
    };
    let bytes = Arc::new(bytes);
    definitions.font_data.insert(
        "system_font".into(),
        Arc::new(egui::FontData::from_owned((*bytes).clone())),
    );
    definitions.font_data.insert(
        "XF_Nstf".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/XF_Nstf.otf"
        ))),
    );
    definitions
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "system_font".into());
    definitions
        .families
        .entry(FontFamily::Name("timetable font".into()))
        .or_default()
        .insert(0, "XF_Nstf".into());
    ctx.global_style_mut(|s| {
        s.text_styles.insert(TextStyle::Name("timetable font".into()), {
            let mut default = FontId::default();
            default.size = 13.0;
            default.family = FontFamily::Name("timetable font".into());
            default
        });
    });
    ctx.set_fonts(definitions);
}
