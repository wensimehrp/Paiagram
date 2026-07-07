use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Margin, Painter, Popup,
    PopupCloseBehavior, Pos2, Rect, Sense, Stroke, Ui, Vec2, WidgetText,
};
use egui_i18n::tr;
use paiagram_core::colors::PredefinedColor;
use paiagram_core::{LonLat, StationKey, XyPos};
use paiagram_core::Wgs84LonLat;
use serde::{Deserialize, Serialize};
use walkers::sources::Attribution;

use crate::tabs::Navigatable;
use crate::tabs::graph::gpu_draw::ShapeInstance;

/// Convert navi (xy) coordinates (Web Mercator meters) to LonLat
fn navi_xy_to_lonlat(x: f64, y: f64) -> LonLat {
    LonLat::from(Wgs84LonLat::from(XyPos { x, y }))
}

/// Convert LonLat to navi (xy) coordinates (Web Mercator meters)
fn lonlat_to_navi_xy(coor: LonLat) -> (f64, f64) {
    let xy: XyPos = Wgs84LonLat::from(coor).into();
    (xy.x, xy.y)
}
use crate::{
    App, CoordinateSelection, ModifySelectedItems, SelectedItem, SelectedItems, StationSelection,
};

mod gpu_draw;
mod underlay;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GraphTab {
    navi: GraphNavigation,
    underlay_tile_type: underlay::UnderlayTileType,
    #[serde(skip, default)]
    underlay_tile_change: Option<underlay::UnderlayTileType>,
    #[serde(skip, default)]
    arrange_iterations: u32,
    #[serde(skip, default)]
    osm_area_name: String,
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuGraphRendererState>>,
}

fn default_arrange_iterations() -> u32 {
    1000
}

impl Default for GraphTab {
    fn default() -> Self {
        Self {
            navi: GraphNavigation::default(),
            underlay_tile_type: underlay::UnderlayTileType::None,
            underlay_tile_change: None,
            arrange_iterations: default_arrange_iterations(),
            osm_area_name: String::new(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuGraphRendererState::default(),
            )),
        }
    }
}

impl PartialEq for GraphTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GraphNavigation {
    x_offset: f64,
    y_offset: f64,
    zoom: f32,
    visible: egui::Rect,
}

impl Default for GraphNavigation {
    fn default() -> Self {
        Self {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: 1.0,
            visible: egui::Rect::NOTHING,
        }
    }
}

impl super::Navigatable for GraphNavigation {
    type XOffset = f64;
    type YOffset = f64;
    fn zoom_x(&self) -> f32 {
        self.zoom
    }
    fn zoom_y(&self) -> f32 {
        self.zoom
    }
    fn set_zoom(&mut self, zoom_x: f32, _zoom_y: f32) {
        self.zoom = zoom_x;
    }
    fn offset_x(&self) -> f64 {
        self.x_offset
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = offset_x;
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible
    }
}

impl super::Tab for GraphTab {
    const NAME: &'static str = "Graph";
    fn title(&self) -> WidgetText {
        tr!("tab-graph").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .outer_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            .show(ui, |ui| display(self, app, ui));
    }

    fn edit_display(&mut self, _app: &mut App, ui: &mut egui::Ui) {
        self.underlay_tile_change = ui
            .add(&mut self.underlay_tile_type)
            .changed()
            .then_some(self.underlay_tile_type);

        ui.separator();
        ui.label(tr!("tab-graph-auto-arrange"));
        ui.label(tr!("tab-graph-auto-arrange-desc"));
        ui.add(
            egui::Slider::new(&mut self.arrange_iterations, 100..=10000)
                .text(tr!("tab-graph-auto-arrange-iterations")),
        );
        if ui.button(tr!("tab-graph-arrange-button")).clicked() {
            // TODO: re-add auto-arrange using new data model
        }

        ui.separator();
        ui.label(tr!("tab-graph-arrange-via-osm"));
        ui.label(tr!("tab-graph-arrange-via-osm-desc"));
        ui.horizontal(|ui| {
            ui.label(tr!("tab-graph-osm-area-name"));
            ui.text_edit_singleline(&mut self.osm_area_name);
        });
        if ui.button(tr!("tab-graph-arrange-button")).clicked() {
            // TODO: re-add OSM arrange
        }
    }
}

fn display(tab: &mut GraphTab, app: &mut App, ui: &mut egui::Ui) {
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
    tab.navi.visible = response.rect;
    tab.navi.handle_navigation(ui, &response);

    // Draw underlay (map tiles)
    let mut underlay_painter = underlay::UnderlayPainter::new();
    underlay_painter.update_tile_type(Some(tab.underlay_tile_type));
    let changed_tile = tab.underlay_tile_change.take();
    underlay_painter.update_tile_type(changed_tile);
    let attribution = underlay_painter.draw_underlay(
        &mut painter,
        &tab.navi,
        ui,
    );

    let mut state = tab.gpu_state.lock();
    if let Some(target_format) = ui.data(|data| {
        data.get_temp::<eframe::egui_wgpu::wgpu::TextureFormat>(
            egui::Id::new("wgpu_target_format"),
        )
    }) {
        state.target_format = Some(target_format);
    }
    if let Some(msaa_samples) =
        ui.data(|data| data.get_temp::<u32>(egui::Id::new("wgpu_msaa_samples")))
    {
        state.msaa_samples = msaa_samples;
    }

    let interact_pos = response
        .clicked()
        .then_some(ui.input(|r| r.pointer.interact_pos()))
        .flatten();

    let selected_item = push_draw_items(
        ui.visuals().dark_mode,
        &tab.navi,
        &mut state.instances,
        &mut painter,
        interact_pos,
        ui.animate_bool(ui.id().with("gugugaga"), tab.navi.zoom > 0.002),
        app,
    );

    let shift_pressed = ui.input(|r| r.modifiers.shift);
    handle_selection_click(
        &selected_item,
        ui.input(|r| r.modifiers.command),
        shift_pressed,
        app,
    );

    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);

    if let Some(attribution) = attribution {
        draw_attribution(ui, response.rect, &attribution);
    }
    draw_scale_bar(
        &painter,
        response.rect,
        tab.navi.zoom,
        ui.visuals().text_color(),
    );

    // Shift+click visual: draw a line from the selected station to cursor
    if shift_pressed {
        if let SelectedItems::Stations(stations) = &app.selected_items {
            if stations.len() == 1 {
                if let Some(cursor_pos) = ui.input(|r| r.pointer.hover_pos()) {
                    let stn = stations.first();
                    if let Some(view) = app.stations.get_view(stn.station) {
                        let (x, y) = lonlat_to_navi_xy(view.pos);
                        let station_pos = tab.navi.xy_to_screen_pos(x, y);
                        painter.line_segment(
                            [station_pos, cursor_pos],
                            Stroke::new(1.0, Color32::RED),
                        );
                    }
                }
            }
        }
    }

    handle_selection_interaction(tab, app, ui, &mut painter, &selected_item, interact_pos);
}

fn handle_selection_click(
    selected_item: &Option<Option<SelectedItem>>,
    command_pressed: bool,
    shift_pressed: bool,
    app: &mut App,
) {
    match selected_item {
        Some(Some(item)) if command_pressed => {
            app.modify_selected_items(ModifySelectedItems::Toggle(item.clone()));
        }
        Some(Some(item)) if shift_pressed => {
            // Shift+click: create interval between existing selection and clicked station
            if let SelectedItem::Station(clicked) = item.clone() {
                let prev_stations: Vec<StationKey> = match &app.selected_items {
                    SelectedItems::Stations(s) => s.iter().map(|st| st.station).collect(),
                    _ => vec![],
                };
                if let Some(&prev) = prev_stations.first() {
                    if prev != clicked.station {
                        if !app.has_edge(prev, clicked.station) {
                            let _ = app.source.apply_command(
                                paiagram_core::Command::AddInterval {
                                    key: paiagram_core::IntervalKey::new(),
                                    view: paiagram_core::IntervalView {
                                        nodes: [
                                            app.stations.get_view(prev).map(|v| v.pos).unwrap_or(paiagram_core::LonLat { lon: 0, lat: 0 }),
                                            app.stations.get_view(clicked.station).map(|v| v.pos).unwrap_or(paiagram_core::LonLat { lon: 0, lat: 0 }),
                                        ].into(),
                                        length: None,
                                    },
                                    from: Some(prev),
                                    to: Some(clicked.station),
                                },
                            );
                        }
                        app.selected_items = SelectedItems::None;
                    }
                }
            }
        }
        Some(Some(item)) => {
            app.modify_selected_items(ModifySelectedItems::SetSingle(item.clone()));
        }
        _ => {}
    }
}

fn handle_selection_interaction(
    tab: &GraphTab,
    app: &mut App,
    ui: &mut Ui,
    painter: &mut Painter,
    selected_item: &Option<Option<SelectedItem>>,
    interact_pos: Option<Pos2>,
) {
    // Handle blank-space clicks: clear selection unless we're already in None state
    if matches!(selected_item, Some(None)) && interact_pos.is_some() {
        match &app.selected_items {
            SelectedItems::None => {
                // Click on empty canvas with nothing selected → create coordinate
                let (x, y) = tab.navi.screen_pos_to_xy(interact_pos.unwrap());
                let coor = navi_xy_to_lonlat(x, y);
                app.selected_items = SelectedItems::Coordinate(CoordinateSelection {
                    coor,
                    name_candidate: String::new(),
                });
                return;
            }
            SelectedItems::Coordinate(_) => {
                // Click on canvas while coordinate popup is open → close popup
                app.selected_items = SelectedItems::None;
                return;
            }
            _ => {
                // Click on canvas while something else is selected → clear
                app.selected_items = SelectedItems::None;
                return;
            }
        }
    }

    // No blank click: handle existing coordinate or other selections
    if matches!(app.selected_items, SelectedItems::None) {
        return;
    }

    // Handle Coordinate: extract owned data first to avoid borrow conflicts with the popup closure
    let coordinate_info = match &app.selected_items {
        SelectedItems::Coordinate(c) => Some((c.coor, c.name_candidate.clone())),
        _ => None,
    };
    if let Some((coor, mut name_candidate)) = coordinate_info {
        let (pos_x, pos_y) = lonlat_to_navi_xy(coor);
        let screen_pos = tab.navi.xy_to_screen_pos(pos_x, pos_y);
        let rect = Rect::from_pos(screen_pos).expand(6.0);
        painter.rect(
            rect,
            0,
            Color32::RED.gamma_multiply(0.5),
            Stroke::new(1.0, Color32::RED),
            egui::StrokeKind::Middle,
        );
        let res = ui
            .allocate_rect(rect, Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab);

        // For drag updates, keep the mutable reference in the selection
        if res.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::Grabbing);
            if let SelectedItems::Coordinate(ref mut c) = app.selected_items {
                let new_screen_pos = screen_pos + res.drag_delta();
                let (x, y) = tab.navi.screen_pos_to_xy(new_screen_pos);
                c.coor = navi_xy_to_lonlat(x, y);
            }
        }

        // Use a raw pointer to write back the name from the popup
        let name_ptr: *mut String = if let SelectedItems::Coordinate(c) = &mut app.selected_items {
            &mut c.name_candidate
        } else {
            unreachable!()
        };
        Popup::menu(&res)
            .open(true)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(200.0);
                let local_name = unsafe { &mut *name_ptr };
                ui.text_edit_singleline(local_name);
                if ui.button(tr!("graph-new-station")).clicked() {
                    let name = (!local_name.is_empty())
                        .then(|| local_name.clone())
                        .unwrap_or_default();
                    let _ = app.source.apply_command(
                        paiagram_core::Command::AddStation {
                            key: paiagram_core::StationKey::new(),
                            name: name.into(),
                            pos: coor,
                        },
                    );
                    app.selected_items = SelectedItems::None;
                }
                ui.small(coor.to_string());
            });

        if interact_pos.is_some() {
            app.selected_items = SelectedItems::None;
        }
        return;
    }

    // Handle Stations (separate borrow scope)
    if matches!(app.selected_items, SelectedItems::Stations(_)) {
        let station_keys: Vec<StationKey> = match &app.selected_items {
            SelectedItems::Stations(s) => s.iter().map(|st| st.station).collect(),
            _ => vec![],
        };
        for sk in station_keys {
            display_station_info(ui, tab, painter, app, sk);
        }
    }
}

fn display_station_info(
    ui: &mut Ui,
    tab: &GraphTab,
    _painter: &Painter,
    app: &mut App,
    station_key: StationKey,
) {
    if let Some(handle) = app.stations.get_handle(station_key) {
        let name = app.stations.get_name(handle);
        let pos = app.stations.get_pos(handle);
        let (x, y) = lonlat_to_navi_xy(pos);
        let screen_pos = tab.navi.xy_to_screen_pos(x, y);

        let rect = Rect::from_pos(screen_pos).expand(8.0);
        let res = ui
            .allocate_rect(rect, Sense::drag())
            .on_hover_cursor(CursorIcon::Grab);
        if res.dragged() {
            ui.set_cursor_icon(egui::CursorIcon::Grabbing);
            let new_pos = screen_pos + res.drag_delta();
            let (x, y) = tab.navi.screen_pos_to_xy(new_pos);
            let _new_coor = navi_xy_to_lonlat(x, y);
            // TODO: station position update - needs a MoveStation command
        }
        Popup::menu(&res)
            .open(true)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(150.0);
                let mut name_str = name.to_string();
                if ui.text_edit_singleline(&mut name_str).lost_focus() {
                    app.source.apply_command(
                        paiagram_core::Command::RenameStation {
                            key: station_key,
                            name: name_str.into(),
                        },
                    );
                }
                ui.small(pos.to_string());
            });
    }
}

fn draw_scale_bar(painter: &Painter, viewport: Rect, zoom: f32, color: egui::Color32) {
    if zoom <= 0.0 || !viewport.is_positive() {
        return;
    }

    let desired_px = 120.0f64;
    let meters_per_px = 1.0 / zoom as f64;
    let raw_meters = desired_px * meters_per_px;
    let bar_meters = round_to_1_2_5(raw_meters).max(1.0);
    let bar_px = (bar_meters as f32 * zoom).max(1.0);

    let margin = 10.0;
    let baseline_y = viewport.bottom() - margin;
    let left_x = viewport.left() + margin;
    let right_x = left_x + bar_px;

    let stroke = Stroke::new(1.6, color);
    painter.line_segment(
        [
            Pos2::new(left_x, baseline_y),
            Pos2::new(right_x, baseline_y),
        ],
        stroke,
    );

    let tick_len = 7.0;
    painter.line_segment(
        [
            Pos2::new(left_x, baseline_y),
            Pos2::new(left_x, baseline_y - tick_len),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(right_x, baseline_y),
            Pos2::new(right_x, baseline_y - tick_len),
        ],
        stroke,
    );

    let mid_tick_len = 5.0;
    for fraction in [0.25f32, 0.5, 0.75] {
        let x = left_x + bar_px * fraction;
        painter.line_segment(
            [
                Pos2::new(x, baseline_y),
                Pos2::new(x, baseline_y - mid_tick_len),
            ],
            stroke,
        );
    }

    painter.text(
        Pos2::new(left_x, baseline_y - tick_len - 3.0),
        Align2::LEFT_BOTTOM,
        format_scale_label(bar_meters),
        FontId::proportional(12.0),
        color,
    );
}

fn round_to_1_2_5(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let exponent = value.log10().floor();
    let base = 10.0f64.powf(exponent);
    let normalized = value / base;
    let rounded = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    rounded * base
}

fn format_scale_label(meters: f64) -> String {
    if meters >= 1000.0 {
        let km = meters / 1000.0;
        if (km - km.round()).abs() < 1e-6 {
            format!("{:.0} km", km)
        } else {
            format!("{:.1} km", km)
        }
    } else {
        format!("{:.0} m", meters)
    }
}

fn draw_attribution(ui: &mut Ui, viewport: Rect, attribution: &Attribution) {
    let margin = 6.0;
    let font_id = FontId::proportional(13.0);
    let color = ui.style().visuals.hyperlink_color;
    let text = format!("© {}", attribution.text);
    let galley = ui.painter().layout_no_wrap(text.clone(), font_id, color);
    let size = galley.size();
    let min = Pos2::new(
        viewport.right() - margin - size.x,
        viewport.bottom() - margin - size.y,
    );
    let rect = Rect::from_min_size(min, size);
    let mut r = CornerRadius::ZERO;
    r.nw = 4;
    ui.painter()
        .rect_filled(rect.expand(margin), r, Color32::WHITE.gamma_multiply(0.5));
    ui.put(
        rect,
        egui::Hyperlink::from_label_and_url(text, attribution.url).open_in_new_tab(true),
    );
}

fn push_draw_items(
    is_dark: bool,
    navi: &GraphNavigation,
    buffer: &mut Vec<ShapeInstance>,
    painter: &mut Painter,
    maybe_interact_pos: Option<Pos2>,
    text_strength: f32,
    app: &App,
) -> Option<Option<SelectedItem>> {
    buffer.clear();

    let selection_strength = painter.ctx().animate_bool_responsive(
        Id::new("graph selection animation"),
        !matches!(app.selected_items, SelectedItems::None),
    );

    // prepare time
    let time = app.timer.read_seconds();
    let repeat_time = app.project_settings.repeat_frequency.0 as f64;
    let _query_time = if repeat_time > 0.0 {
        time.rem_euclid(repeat_time)
    } else {
        time
    };

    let draw_name = |name: Option<&str>, pos: Pos2, color: Color32| {
        if text_strength > 0.05 && let Some(name) = name {
            painter.text(
                pos + Vec2 { x: 7.0, y: 0.0 },
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(13.0),
                color.gamma_multiply(text_strength),
            );
        }
    };

    let mut selected_item: Option<SelectedItem> = None;
    macro_rules! push_selected_item {
        ($f:expr, $p:pat) => {
            if let Some(interact_pos) = maybe_interact_pos
                && selected_item.is_none()
                && matches!(app.selected_items, SelectedItems::None | $p)
                && let Some(candidate_item) = $f(interact_pos)
            {
                selected_item = Some(candidate_item);
            }
        };
    }

    let color = PredefinedColor::Neutral.into_color32(is_dark);
    let margin_x = 12.0 / navi.zoom_x().max(f32::EPSILON) as f64;
    let margin_y = 12.0 / navi.zoom_y().max(f32::EPSILON) as f64;
    let visible_x = navi.visible_x();
    let visible_y = navi.visible_y();
    let min_x = visible_x.start - margin_x;
    let max_x = visible_x.end + margin_x;
    let min_y = visible_y.start - margin_y;
    let max_y = visible_y.end + margin_y;

    const STATION_SELECTION_RADIUS: f32 = 10.0;
    const SELECTION_RADIUS: f32 = 10.0;

    // Draw intervals
    for interval_key in app.intervals.keys() {
        if let Some(view) = app.intervals.get_view(*interval_key) {
            let points = &view.nodes;
            for pair in points.windows(2) {
                let (x0, y0) = lonlat_to_navi_xy(pair[0]);
                let (x1, y1) = lonlat_to_navi_xy(pair[1]);

                if x0 < min_x && x1 < min_x { continue; }
                if x0 > max_x && x1 > max_x { continue; }
                if y0 < min_y && y1 < min_y { continue; }
                if y0 > max_y && y1 > max_y { continue; }

                let spos = navi.xy_to_screen_pos(x0, y0);
                let tpos = navi.xy_to_screen_pos(x1, y1);
                buffer.push(gpu_draw::ShapeInstance::segment(spos, tpos, 1.0, color));
            }
        }
    }

    // Collect visible station candidates
    let candidate_stations: Vec<StationKey> = app
        .stations
        .keys()
        .copied()
        .filter(|k| {
            app.stations.get_view(*k).map_or(false, |view| {
                let (x, y) = lonlat_to_navi_xy(view.pos);
                x >= min_x && x <= max_x && y >= min_y && y <= max_y
            })
        })
        .collect();

    // Draw station selection highlights
    let selected_stations: Vec<StationKey> = match &app.selected_items {
        SelectedItems::Stations(list) => list.iter().map(|s| s.station).collect(),
        _ => Vec::new(),
    };
    for stn_key in &selected_stations {
        if let Some(view) = app.stations.get_view(*stn_key) {
            let (x, y) = lonlat_to_navi_xy(view.pos);
            let pos = navi.xy_to_screen_pos(x, y);
            painter.circle(
                pos,
                SELECTION_RADIUS,
                Color32::RED
                    .gamma_multiply(0.5),
                Stroke::new(1.0, Color32::RED.gamma_multiply(0.8)),
            );
        }
    }

    for stn_key in &candidate_stations {
        if let Some(view) = app.stations.get_view(*stn_key) {
            let (x, y) = lonlat_to_navi_xy(view.pos);
            let screen_pos = navi.xy_to_screen_pos(x, y);

            push_selected_item!(
                |pos| {
                    let r = Rect::from_pos(screen_pos).expand(STATION_SELECTION_RADIUS);
                    r.contains(pos).then_some(SelectedItem::Station(StationSelection {
                        station: *stn_key,
                    }))
                },
                SelectedItems::Stations(_)
            );

            buffer.push(gpu_draw::ShapeInstance::circle(screen_pos, 4.0, color));
            draw_name(Some(view.name.as_str()), screen_pos, color);
        }
    }

    maybe_interact_pos.map(|_| selected_item)
}
