use std::sync::{Arc, LazyLock};

use egui::mutex::RwLock;
use egui::{Context, FontDefinitions, FontFamily, FontId, TextStyle};
use fontdb::{Database, Family, ID};
use log::{info, warn};
use parking_lot::Mutex;

pub(crate) static FONT_DATABASE: LazyLock<RwLock<Database>> = LazyLock::new(|| {
    let mut db = Database::new();
    db.load_system_fonts();
    RwLock::new(db)
});

#[cfg(target_arch = "wasm32")]
const WASM_FONT_NAME: &str = "SarasaUiCL-Regular.ttf";

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

pub(crate) fn load_default_font(ctx: Context, font_name: Arc<Mutex<String>>) {
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
    info!("Loaded XF_Nstf");
    ctx.set_fonts(definitions.clone());
    egui_material_icons::initialize(&ctx);
    // dynamic query
    match FONT_DATABASE.read().query(&fontdb::Query {
        families: &[Family::Name("Sarasa UI SC"), Family::SansSerif],
        ..Default::default()
    }) {
        Some(face_id) => {
            load_font_name(face_id, font_name.clone());
            load_font_to_egui(face_id, ctx.clone(), font_name.clone(), definitions);
        }
        None => {
            warn!("Couldn't select font from system");
            let mut name = font_name.lock();
            name.clear();
            name.push_str("Error");
        }
    };
    #[cfg(target_arch = "wasm32")]
    load_sarasa_cl(ctx.clone(), font_name.clone());
}

#[cfg(target_arch = "wasm32")]
fn load_sarasa_cl(ctx: Context, font_name: Arc<Mutex<String>>) {
    wasm_bindgen_futures::spawn_local(async move {
        info!("Downloading `{WASM_FONT_NAME}`");
        let res = match ehttp::fetch_async(ehttp::Request::get(WASM_FONT_NAME)).await {
            Err(e) => {
                warn!("Failed to fetch font `{WASM_FONT_NAME}`. Reason: {e}");
                return;
            }
            Ok(res) => res,
        };
        if !res.ok {
            warn!(
                "Failed to fetch font `{WASM_FONT_NAME}`. Got status code `{}`",
                res.status
            );
        }
        let face_id =
            FONT_DATABASE.write().load_font_source(fontdb::Source::Binary(Arc::new(res.bytes)))[0];
        load_font_to_egui(
            face_id,
            ctx.clone(),
            font_name,
            ctx.fonts(|r| r.definitions().clone()),
        );
    });
}

fn load_font_name(face_id: ID, font_name: Arc<Mutex<String>>) {
    let font_name_clone = font_name.clone();
    FONT_DATABASE.read().face(face_id).map_or_else(
        move || {
            let mut font_name = font_name.lock();
            font_name.clear();
            font_name.push_str("UNNAMED FONT");
        },
        move |info| {
            info!("Trying to load font `{}`", &info.post_script_name);
            let mut font_name = font_name_clone.lock();
            font_name.clear();
            font_name.push_str(&info.post_script_name);
        },
    );
}

pub(crate) fn load_font_to_egui(
    face_id: ID,
    ctx: Context,
    font_name: Arc<Mutex<String>>,
    mut definitions: FontDefinitions,
) {
    let Some(bytes) =
        FONT_DATABASE.read().with_face_data(face_id, |font_bytes, _index| font_bytes.to_owned())
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
    load_font_name(face_id, font_name);
    ctx.set_fonts(definitions);
    ctx.request_repaint();
}
