use std::sync::{Arc, LazyLock};

use egui::{FontDefinitions, FontFamily, FontId, TextStyle};
use fontdb::{Database, Family, ID};
use log::{info, warn};

pub(crate) static FONT_DATABASE: LazyLock<Database> = LazyLock::new(|| {
    let mut db = Database::new();
    #[cfg(target_arch = "wasm32")]
    {
        if load_browser_fonts(&mut db).is_none() {
            warn!("Failed to load browser fonts. Perhaps your browser doesn't support it.");
            warn!(
                "See `https://developer.mozilla.org/en-US/docs/Web/API/Local_Font_Access_API` For compatibilty info"
            );
        }
    }
    db.load_system_fonts();
    db
});

/// Load browser fonts via the local font access api.
/// See <https://developer.mozilla.org/en-US/docs/Web/API/Local_Font_Access_API>.
/// This basically only works on Chromium browsers.
#[cfg(target_arch = "wasm32")]
fn load_browser_fonts(db: &mut Database) -> Option<()> {
    use web_sys;
    let available_fonts = web_sys::window()?.query_local_fonts().ok()?;
    // TODO
    Some(())
}

pub(crate) const TIMETABLTE_TEXT_STYLE: LazyLock<TextStyle> =
    LazyLock::new(|| TextStyle::Name("timetable font".into()));

pub(crate) fn load_default_font(ctx: &egui::Context) -> &str {
    let mut definitions = FontDefinitions::default();
    definitions
        .families
        .entry(FontFamily::Name("timetable font".into()))
        .or_default()
        .insert(0, "XF_Nstf".into());
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(TIMETABLTE_TEXT_STYLE.clone(), {
            let mut default = FontId::default();
            default.size = 13.0;
            default.family = FontFamily::Name("timetable font".into());
            default
        });
    });
    definitions.font_data.insert(
        "XF_Nstf".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/XF_Nstf.otf"
        ))),
    );
    warn!("{:#?}", ctx.global_style().text_styles());
    info!("Loaded XF_Nstf");
    ctx.set_fonts(definitions.clone());
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
    load_font_to_egui(face_id, ctx, definitions);
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
    definitions
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "system_font".into());
    ctx.set_fonts(definitions);
}
