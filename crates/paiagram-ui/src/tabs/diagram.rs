use super::{Navigatable, Tab};
use crate::tabs::station::StationTab;
use crate::widgets::indicators::display_time_indicator_indicator_horizontal;
use crate::widgets::timetable_popup::{POPUP_WIDTH, arrival_popup, departure_popup};
use crate::widgets::{TimeDragValue, buttons};
use crate::{
    EntrySelection, ExtendingTripSelection, GlobalTimer, IntervalSelection, ModifySelectedItems,
    OpenOrFocus, SelectedItem, SelectedItems, StationSelection,
};
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use egui::emath::Numeric;
use egui::epaint::TextShape;
use egui::{
    Align2, Button, Color32, Id, Margin, Painter, Pos2, Rect, RectAlign, Sense, Stroke, Ui, Vec2,
    vec2,
};
use egui_i18n::tr;
use instant::Instant;
use itertools::Itertools;
use moonshine_core::prelude::MapEntities;
use paiagram_core::entry::{
    AdjustEntryMode, EntryEstimate, EntryMode, EntryModeAdjustment, EntryQuery, TravelMode,
};
use paiagram_core::export::ExportObject;
use paiagram_core::route::Route;
use paiagram_core::settings::ProjectSettings;
use paiagram_core::station::Station;
use paiagram_core::trip::class::DisplayedStroke;
use paiagram_core::trip::{Trip, TripBundle, TripClass, TripQuery};
use paiagram_core::units::time::{Duration, Tick, TimetableTime};
use paiagram_raptor::Journey;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::sync::Arc;
pub mod calc_trip_lines;
mod draw_lines;
mod gpu_draw;

impl SelectedItems {
    pub fn to_canvas_state(&mut self) -> CanvasState<'_> {
        match self {
            Self::None => CanvasState::Idle,
            Self::Entries(i) => CanvasState::SelectingEntries(i),
            Self::Intervals(i) => CanvasState::SelectingIntervals(i),
            Self::Stations(i) => CanvasState::SelectingStations(i),
            Self::ExtendingRoute(_) => CanvasState::IdleNoInterrupt,
            Self::ExtendingTrip(i) => CanvasState::ExtendingTrip(i),
        }
    }
}

/// The state of the canvas
#[derive(Default)]
#[non_exhaustive]
pub(crate) enum CanvasState<'a> {
    /// User is doing nothing
    #[default]
    Idle,
    /// User is doing something in another panel.
    IdleNoInterrupt,
    /// User is selecting some entries
    SelectingEntries(&'a [EntrySelection]),
    /// User is selecting some intervals
    SelectingIntervals(&'a [IntervalSelection]),
    /// User is selecting some stations
    SelectingStations(&'a [StationSelection]),
    /// User is extending a trip
    ExtendingTrip(&'a mut ExtendingTripSelection),
}

type TripCache = EntityHashMap<SmallVec<[Vec<TripPoint>; 1]>>;

/// The diagram tab.
#[derive(Serialize, Deserialize, Clone, MapEntities)]
pub struct DiagramTab {
    /// The navigation info
    navi: DiagramTabNavigation,
    /// In the case where the user secondary clicked on the page, where?
    #[serde(skip, default)]
    last_secondary_click_position: Option<(Tick, f64)>,
    /// The route's entity
    #[entities]
    route_entity: Entity,
    /// Whether to use the [`GlobalTimer`]
    use_global_timer: bool,
    #[serde(skip, default)]
    cached_trips: Option<TripCache>,
    /// RAPTOR's results
    #[serde(skip, default)]
    raptor_params: RaptorParams,
    #[serde(skip, default)]
    times: (f32, f32, f32),
    /// GPU state for drawing the lines
    #[serde(skip, default)]
    gpu_state: Arc<egui::mutex::Mutex<gpu_draw::GpuTripRendererState>>,
}

#[derive(Clone, Default)]
pub struct RaptorParams {
    departure_time: TimetableTime,
    start_stop: Option<Entity>,
    end_stop: Option<Entity>,
    result: Vec<Journey<Entity, Entity>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiagramTabNavigation {
    pub x_offset: Tick,
    pub y_offset: f64,
    pub zoom: Vec2,
    #[serde(skip, default = "default_visible_rect")]
    pub visible_rect: Rect,
    // cache zone
    pub max_height: f32,
}

impl Default for DiagramTabNavigation {
    fn default() -> Self {
        Self {
            x_offset: Tick(0),
            y_offset: 0.0,
            zoom: vec2(0.005, 10.0),
            visible_rect: Rect::NOTHING,
            max_height: 0.0,
        }
    }
}

fn default_visible_rect() -> Rect {
    Rect::NOTHING
}

impl PartialEq for DiagramTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_entity == other.route_entity
    }
}

impl DiagramTab {
    pub fn new(route_entity: Entity) -> Self {
        Self {
            navi: DiagramTabNavigation::default(),
            last_secondary_click_position: None,
            route_entity,
            use_global_timer: false,
            cached_trips: None,
            times: (0.0, 0.0, 0.0),
            raptor_params: RaptorParams::default(),
            gpu_state: Arc::new(egui::mutex::Mutex::new(
                gpu_draw::GpuTripRendererState::default(),
            )),
        }
    }
}

impl Navigatable for DiagramTabNavigation {
    type XOffset = paiagram_core::units::time::Tick;
    type YOffset = f64;

    fn zoom_x(&self) -> f32 {
        self.zoom.x
    }
    fn zoom_y(&self) -> f32 {
        self.zoom.y
    }
    fn set_zoom(&mut self, zoom_x: f32, zoom_y: f32) {
        self.zoom = Vec2::new(zoom_x, zoom_y);
    }
    fn offset_x(&self) -> f64 {
        self.x_offset.0 as f64
    }
    fn offset_y(&self) -> f64 {
        self.y_offset
    }
    fn set_offset(&mut self, offset_x: f64, offset_y: f64) {
        self.x_offset = Tick(offset_x.round() as i64);
        self.y_offset = offset_y;
    }
    fn visible_rect(&self) -> egui::Rect {
        self.visible_rect
    }
    fn x_per_screen_unit(&self) -> Self::XOffset {
        Tick((1.0 / self.zoom_x().max(f32::EPSILON) as f64) as i64)
    }
    fn visible_x(&self) -> std::ops::Range<Self::XOffset> {
        let width = self.visible_rect().width() as f64;
        let ticks_per_screen_unit = 1.0 / self.zoom_x().max(f32::EPSILON) as f64;
        let start = self.x_offset;
        let end = Tick(start.0 + (width * ticks_per_screen_unit).ceil() as i64);
        start..end
    }
    fn visible_y(&self) -> std::ops::Range<Self::YOffset> {
        let height = self.visible_rect.height() as f64;
        let start = self.offset_y();
        let end = start + height / self.zoom_y().max(f32::EPSILON) as f64;
        start..end
    }
    fn y_per_screen_unit(&self) -> Self::YOffset {
        1.0 / self.zoom_y().max(f32::EPSILON) as f64
    }
    fn allow_axis_zoom(&self) -> bool {
        true
    }
    fn clamp_zoom(&self, zoom_x: f32, zoom_y: f32) -> (f32, f32) {
        (zoom_x.clamp(0.00005, 0.4), zoom_y.clamp(0.1, 2048.0))
    }
    fn post_navigation(&mut self, response: &egui::Response) {
        let max_tick = Tick::from_timetable_time(TimetableTime(366 * 86400)).0;
        self.x_offset = Tick(self.x_offset.0.clamp(
            -max_tick,
            max_tick - (response.rect.width() as f64 / self.zoom.x as f64) as i64,
        ));
        const TOP_BOTTOM_PADDING: f32 = 30.0;
        self.y_offset = if response.rect.height() / self.zoom.y
            > (self.max_height + TOP_BOTTOM_PADDING * 2.0 / self.zoom.y)
        {
            ((-response.rect.height() / self.zoom.y + self.max_height) / 2.0) as f64
        } else {
            self.y_offset.clamp(
                (-TOP_BOTTOM_PADDING / self.zoom.y) as f64,
                (self.max_height - response.rect.height() / self.zoom.y
                    + TOP_BOTTOM_PADDING / self.zoom.y) as f64,
            )
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TripPoint {
    arr: TimetableTime,
    dep: TimetableTime,
    entry: Entity,
    station_index: usize,
}

fn render_points(
    (InRef(cache), InMut(buf), InRef(navi), InRef(station_heights)): (
        InRef<TripCache>,
        InMut<Vec<DrawnTrip>>,
        InRef<DiagramTabNavigation>,
        InRef<[(Entity, f32)]>,
    ),
    trip_class_q: Query<&TripClass>,
    stroke_q: Query<&DisplayedStroke>,
    settings: Res<ProjectSettings>,
) {
    if buf.len() < cache.len() {
        buf.resize_with(cache.len(), DrawnTrip::empty);
    }
    let visible_ticks = navi.visible_x();
    let repeat_freq_ticks = Tick::from_timetable_time(TimetableTime(settings.repeat_frequency.0));
    for ((trip_entry, lines), buf) in cache.iter().zip(buf.iter_mut()) {
        let class = trip_class_q.get(*trip_entry).unwrap();
        let stroke = stroke_q.get(class.entity()).unwrap();
        buf.entity = *trip_entry;
        buf.stroke = *stroke;
        let mut drawn_count = 0;
        for line in lines.iter() {
            let mut line_min = i64::MAX;
            let mut line_max = i64::MIN;
            for TripPoint { arr, dep, .. } in line.iter() {
                let arr_ticks = Tick::from_timetable_time(*arr).0;
                let dep_ticks = Tick::from_timetable_time(*dep).0;
                line_min = line_min.min(arr_ticks.min(dep_ticks));
                line_max = line_max.max(arr_ticks.max(dep_ticks));
            }
            if line_min > line_max {
                continue;
            }

            let (repeat_start, repeat_end) = if repeat_freq_ticks.0 > 0 {
                (
                    (visible_ticks.start.0 - line_max).div_euclid(repeat_freq_ticks.0),
                    (visible_ticks.end.0 - line_min).div_euclid(repeat_freq_ticks.0),
                )
            } else {
                (0, 0)
            };

            for repeat in repeat_start..=repeat_end {
                if drawn_count >= buf.points.len() {
                    buf.points.resize_with(drawn_count + 1, Vec::new);
                }
                if drawn_count >= buf.entries.len() {
                    buf.entries.resize_with(drawn_count + 1, Vec::new);
                }
                let pt_buf = &mut buf.points[drawn_count];
                let et_buf = &mut buf.entries[drawn_count];

                pt_buf.clear();
                et_buf.clear();
                et_buf.reserve(line.len());
                pt_buf.reserve(line.len());

                let repeat_offset = repeat * repeat_freq_ticks.0;
                for TripPoint {
                    arr,
                    dep,
                    entry,
                    station_index,
                } in line.iter()
                {
                    let (_, height) = station_heights[*station_index];
                    let arr_tick = Tick::from_timetable_time(*arr);
                    let dep_tick = Tick::from_timetable_time(*dep);
                    let arr_pos =
                        navi.xy_to_screen_pos(Tick(arr_tick.0 + repeat_offset), height as f64);
                    let dep_pos =
                        navi.xy_to_screen_pos(Tick(dep_tick.0 + repeat_offset), height as f64);
                    et_buf.push(*entry);
                    pt_buf.push([arr_pos, arr_pos, dep_pos, dep_pos]);
                }

                if !pt_buf.is_empty() {
                    drawn_count += 1;
                }
            }
        }
        buf.draw_amount = drawn_count;
        buf.points.truncate(drawn_count);
        buf.entries.truncate(drawn_count);
    }
}

#[derive(Debug)]
pub struct DrawnTrip {
    pub entity: Entity,
    pub stroke: DisplayedStroke,
    pub draw_amount: usize,
    pub points: Vec<Vec<[Pos2; 4]>>,
    pub entries: Vec<Vec<Entity>>, // TODO: remove this field
}

impl DrawnTrip {
    fn empty() -> Self {
        Self {
            entity: Entity::PLACEHOLDER,
            stroke: DisplayedStroke::from_seed([0]),
            draw_amount: 0,
            points: Vec::new(),
            entries: Vec::new(),
        }
    }
}

impl Tab for DiagramTab {
    const NAME: &'static str = "Diagram";
    fn id(&self) -> Id {
        Id::new(self.route_entity)
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn export_display(&mut self, world: &mut World, ui: &mut Ui) {
        use crate::export_typst_diagram::{TypstDiagram, TypstModule};
        use paiagram_core::export::oudia::OuDia;
        ui.strong(tr!("tab-diagram-save-typst-module"));
        ui.label(tr!("tab-diagram-save-typst-module-desc"));
        if ui.button(tr!("export")).clicked() {
            TypstModule.export_to_file();
        }
        ui.strong(tr!("tab-diagram-export-json-data"));
        ui.label(tr!("tab-diagram-export-json-data"));
        if ui.button(tr!("export")).clicked() {
            TypstDiagram {
                route_entity: self.route_entity,
                world: world,
            }
            .export_to_file();
        }
        if ui.button("Export to OuDia").clicked() {
            OuDia {
                route_entity: self.route_entity,
                world: world,
            }
            .export_to_file();
        }
    }
    fn edit_display(&mut self, world: &mut World, ui: &mut Ui) {
        ui.monospace(format!("trip calc:  {}", self.times.0));
        ui.monospace(format!("screen pos: {}", self.times.1));
        ui.monospace(format!("rendering:  {}", self.times.2));
        ui.checkbox(&mut self.use_global_timer, "Use global timer");
        let selected = world.resource::<SelectedItems>().clone();
        match selected {
            SelectedItems::None => {
                ui.strong("New Trip");
                ui.label("Create a new trip from scratch");
                if ui.button("Create a new trip").clicked() {
                    let default_class = world
                        .resource::<paiagram_core::class::ClassResource>()
                        .default_class;
                    let new_trip = world
                        .commands()
                        .spawn(TripBundle::new(
                            "New Trip",
                            TripClass(default_class),
                            Vec::new(),
                        ))
                        .id();
                    *world.resource_mut::<SelectedItems>() =
                        SelectedItems::ExtendingTrip(ExtendingTripSelection {
                            entry: new_trip,
                            previous_pos: None,
                            current_entry: None,
                            last_time: None,
                        })
                }
            }
            SelectedItems::ExtendingTrip(ExtendingTripSelection {
                entry,
                previous_pos: _,
                current_entry: _,
                last_time: _,
            }) => {
                let mut name = world.get_mut::<Name>(entry).unwrap();
                name.mutate(|n| {
                    ui.text_edit_singleline(n);
                });
                if ui.button("Complete").clicked() {
                    *world.resource_mut::<SelectedItems>() = SelectedItems::None
                }
            }
            SelectedItems::Entries(selected_entries) => {
                // world
                //     .run_system_cached_with(display_entry_info, (ui, selected_entries.as_slice()))
                //     .unwrap();
            }
            SelectedItems::Intervals(_) => {}
            SelectedItems::Stations(_) => {}
            SelectedItems::ExtendingRoute(_) => {}
        }
        ui.separator();
    }
    fn display_display(&mut self, world: &mut World, ui: &mut Ui) {
        ui.label("Find a route between...");
        ui.add(
            egui::Slider::new(
                &mut self.raptor_params.departure_time,
                TimetableTime(0)..=TimetableTime(86400),
            )
            .custom_formatter(|val, _| TimetableTime::from_f64(val).to_string())
            .custom_parser(|s| TimetableTime::from_str(s).map(TimetableTime::to_f64)),
        );
        // station selection
        // TODO: support both select from canvas and select from station list
        // TODO: make this a modal instead of this list thingy
        fn select_name(
            (InMut(ui), InMut(sel)): (InMut<Ui>, InMut<Option<Entity>>),
            stations: Query<(Entity, &Name), With<Station>>,
        ) {
            let res = ui.button(
                sel.and_then(|sel| stations.get(sel).ok())
                    .map_or("None", |(_, n)| n.as_str()),
            );
            egui::Popup::menu(&res).show(|ui| {
                egui::scroll_area::ScrollArea::vertical().show(ui, |ui| {
                    for (entity, station_name) in stations.iter() {
                        if ui.button(station_name.as_str()).clicked() {
                            *sel = Some(entity)
                        }
                    }
                })
            });
        }
        world
            .run_system_cached_with(select_name, (ui, &mut self.raptor_params.start_stop))
            .unwrap();
        world
            .run_system_cached_with(select_name, (ui, &mut self.raptor_params.end_stop))
            .unwrap();
        if ui
            .add_enabled(
                self.raptor_params.start_stop.is_some() && self.raptor_params.end_stop.is_some(),
                egui::Button::new("Find"),
            )
            .clicked()
            && let Some(start) = self.raptor_params.start_stop
            && let Some(end) = self.raptor_params.end_stop
        {
            self.raptor_params.result = world
                .run_system_cached_with(
                    paiagram_raptor::make_query_data,
                    (self.raptor_params.departure_time.0 as usize, start, end),
                )
                .unwrap();
        }
        // TODO: highlight the trips on the canvas instead of displaying them here
        fn display_journey(
            (InMut(ui), InRef(journeys)): (InMut<Ui>, InRef<[Journey<Entity, Entity>]>),
            name_q: Query<&Name>,
        ) {
            for (idx, Journey { plan, arrival }) in journeys.iter().enumerate() {
                egui::Grid::new(("journey grid", idx))
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Arrival Time:");
                        ui.label(TimetableTime(*arrival as i32).to_string());
                        ui.end_row();
                        for (route, stop) in plan.iter().copied() {
                            let stop_n = name_q.get(stop).map_or("No Name", Name::as_str);
                            let route_n = name_q.get(route).map_or("No Name", Name::as_str);
                            ui.label(stop_n);
                            ui.label(route_n);
                            ui.end_row();
                        }
                    });
                ui.separator();
            }
        }

        world
            .run_system_cached_with(display_journey, (ui, self.raptor_params.result.as_slice()))
            .unwrap();
    }
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        world.resource_scope(|world, mut selected: Mut<SelectedItems>| {
            egui::Frame::canvas(ui.style())
                .inner_margin(Margin::ZERO)
                .outer_margin(Margin::ZERO)
                .stroke(Stroke::NONE)
                .show(ui, |ui| {
                    main_display(self, world, ui, selected.to_canvas_state())
                });
        });
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
pub(crate) struct TripLineBuf(Vec<DrawnTrip>);

fn main_display(
    tab: &mut DiagramTab,
    world: &mut World,
    ui: &mut egui::Ui,
    canvas_state: CanvasState,
) {
    let mut trip_line_buf: Vec<DrawnTrip> = Vec::new();
    let trip_line_buf = &mut trip_line_buf;
    let route = world
        .get::<Route>(tab.route_entity)
        .expect("Entity should have a route");

    // Setup the response and the painter
    let (response, mut painter) =
        ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());

    // Timer shifting logic
    let timer = world.resource::<GlobalTimer>();
    tab.navi.visible_rect = response.rect;
    if tab.use_global_timer {
        tab.navi.x_offset = timer.read_ticks();
    }
    let moved = tab.navi.handle_navigation(ui, &response);
    if tab.use_global_timer {
        timer.write_ticks(tab.navi.x_offset);
    }
    if moved && tab.use_global_timer {
        timer.try_lock(tab.route_entity);
    } else {
        timer.try_unlock(tab.route_entity);
    }

    // Prepare the station info
    let station_heights: Vec<_> = route.iter().collect();
    if station_heights.is_empty() {
        return;
    }
    tab.navi.max_height = station_heights.last().map_or(0.0, |(_, h)| *h);

    // Draw the horizontal station lines
    draw_lines::draw_station_lines(
        &mut painter,
        &tab.navi,
        station_heights.iter().copied(),
        ui.visuals(),
        &world,
    );

    // Draw the vertical time lines
    draw_lines::draw_time_lines(&mut painter, &tab.navi);

    // Calculate the visible trains
    let now = Instant::now();
    let cached_trips_are_changed = world
        .run_system_cached_with(
            calc_trip_lines::calc,
            (tab.route_entity, &station_heights, &mut tab.cached_trips),
        )
        .unwrap();
    tab.times.0 = tab.times.0 * 0.9 + now.elapsed().as_secs_f32() * 1000.0 * 0.1;

    // Prepare GPU drawing
    let now = Instant::now();
    let mut state = tab.gpu_state.lock();

    if let Some(target_format) = ui.ctx().data(|data| {
        data.get_temp::<eframe::egui_wgpu::wgpu::TextureFormat>(Id::new("wgpu_target_format"))
    }) {
        state.target_format = Some(target_format);
    }
    if let Some(msaa_samples) = ui
        .ctx()
        .data(|data| data.get_temp::<u32>(Id::new("wgpu_msaa_samples")))
    {
        state.msaa_samples = msaa_samples;
    }
    let repeat_frequency = world.resource::<ProjectSettings>().repeat_frequency;

    state.uniforms = gpu_draw::Uniforms {
        ticks_min: tab.navi.x_offset.0,
        y_min: tab.navi.y_offset,
        screen_size: [0.0, 0.0],
        x_per_unit: tab.navi.x_per_screen_unit_f64() as f32,
        y_per_unit: tab.navi.y_per_screen_unit_f64() as f32,
        screen_origin: [tab.navi.visible_rect.left(), tab.navi.visible_rect.top()],
        repeat_interval_ticks: Tick::from_timetable_time(TimetableTime(repeat_frequency.0))
            .0
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
    };

    if cached_trips_are_changed {
        gpu_draw::rewrite_trip_cache(
            tab.cached_trips.as_ref().unwrap(),
            station_heights.iter().map(|(_, y)| *y),
            &mut state,
        );
    }

    let mut processed = 0usize;
    for (trip_entity, repeat_count) in tab
        .cached_trips
        .as_ref()
        .unwrap()
        .iter()
        .map(|(k, v)| (*k, v.len()))
    {
        let class = world.get::<TripClass>(trip_entity).unwrap();
        let stroke = world.get::<DisplayedStroke>(class.0).unwrap();
        let stroke = stroke.egui_stroke(ui.visuals().dark_mode);
        for idx in 0..repeat_count {
            let state_trip = &mut state.trips[idx + processed];
            state_trip.color = stroke.color.to_normalized_gamma_f32();
            state_trip.width = stroke.width;
        }
        processed += repeat_count;
    }

    let callback = gpu_draw::paint_callback(response.rect, tab.gpu_state.clone());
    painter.add(callback);
    tab.times.2 = tab.times.2 * 0.9 + now.elapsed().as_secs_f32() * 1000.0 * 0.1;

    // check for selection
    let selection_strength = ui.ctx().animate_bool(
        ui.id().with("selection"),
        !matches!(canvas_state, CanvasState::Idle),
    );

    let s = (selection_strength * 0.5 * u8::MAX as f32) as u8;
    painter.rect_filled(
        response.rect,
        0,
        if ui.visuals().dark_mode {
            Color32::from_black_alpha(s)
        } else {
            Color32::from_white_alpha(s)
        },
    );

    let interact_pos = response
        .clicked()
        .then(|| ui.input(|it| it.pointer.interact_pos()))
        .flatten();

    match canvas_state {
        // The current canvas is idle.
        // When idle, the user should be able to select intervals, entries, and stations
        // TODO: replace this with if let guard
        CanvasState::Idle if interact_pos.is_some() => {
            let pos = interact_pos.unwrap();
            // state transformation
            error!("INTERACTION IS UNIMPLEMENTED");
            // TODO: read the status from the GPU.
            // then select stuff
            // if let Some(selection) = select_trip(&trip_line_buf, pos) {
            //     world.write_message(ModifySelectedItems::SetSingle(SelectedItem::Entries(
            //         selection,
            //     )));
            // } else if false {
            //     // TODO
            // } else if false {
            //     // TODO
            // }
            // also reset the secondary click memory
            tab.last_secondary_click_position = None;
        }
        CanvasState::IdleNoInterrupt if interact_pos.is_some() => {
            tab.last_secondary_click_position = None;
        }
        CanvasState::Idle | CanvasState::IdleNoInterrupt if response.secondary_clicked() => {
            let pos = ui.input(|it| it.pointer.interact_pos()).unwrap();
            tab.last_secondary_click_position = Some(tab.navi.screen_pos_to_xy(pos));
        }
        CanvasState::Idle | CanvasState::IdleNoInterrupt
            if tab.last_secondary_click_position.is_some() =>
        {
            let (x, y) = tab.last_secondary_click_position.unwrap();
            // Determine the closest station and get its entity and height
            let (closest_station, station_y) = {
                let selected_y = y as f32;
                let idx = station_heights.partition_point(|(_, y)| *y < selected_y);
                if idx == 0 {
                    station_heights.first().copied()
                } else if idx >= station_heights.len() {
                    station_heights.last().copied()
                } else {
                    let (prev_e, prev_y) = station_heights[idx - 1];
                    let (curr_e, curr_y) = station_heights[idx];
                    if selected_y > (prev_y + curr_y) / 2.0 {
                        Some((curr_e, curr_y))
                    } else {
                        Some((prev_e, prev_y))
                    }
                }
            }
            .unwrap();
            let station_y = tab.navi.logical_y_to_screen_y(station_y as f64);
            let screen_pos = tab.navi.xy_to_screen_pos(x, y);

            // position indicator
            painter.line_segment(
                [screen_pos, Pos2::new(screen_pos.x, station_y)],
                Stroke::new(1.0, Color32::RED),
            );
            painter.circle_filled(screen_pos, 3.0, Color32::RED);

            // allocate a new rect to show the popup
            let rect = Rect::from_pos(screen_pos).expand(6.0);
            let res = ui
                .allocate_rect(rect, Sense::drag())
                .on_hover_cursor(egui::CursorIcon::Grab);
            if res.dragged() {
                ui.set_cursor_icon(egui::CursorIcon::Grabbing);
                let new_pos = screen_pos + res.drag_delta();
                tab.last_secondary_click_position = Some(tab.navi.screen_pos_to_xy(new_pos));
            }

            // the popup secondary menu
            egui::Popup::menu(&res).open(true).show(|ui| {
                // Display the station name and open the station tab when clicked.
                ui.set_min_width(POPUP_WIDTH);
                let station_name = world.get::<Name>(closest_station).unwrap().as_str();
                if ui.add(Button::new(station_name).truncate()).clicked() {
                    // also reset the secondary click memory
                    tab.last_secondary_click_position = None;
                    world.write_message(OpenOrFocus(crate::MainTab::Station(StationTab::new(
                        closest_station,
                    ))));
                };

                // convenient DragValue for adjusting the time
                let mut new_time = x.to_timetable_time();
                if ui.add(TimeDragValue(&mut new_time)).changed() {
                    tab.last_secondary_click_position =
                        Some((Tick::from_timetable_time(new_time), y))
                }

                // Add a new trip.
                if matches!(canvas_state, CanvasState::IdleNoInterrupt) {
                    ui.label("Already editing...");
                } else if ui.button("New Trip").clicked() {
                    // also reset the secondary click memory
                    tab.last_secondary_click_position = None;
                    // TODO
                }
            });
        }
        CanvasState::Idle | CanvasState::IdleNoInterrupt => {
            // Do nothing now. there is nothing to handle
        }
        // The current canvas is not idle
        // If the user has already selected some entries, they should only be able to select more
        // entries, or quit the current state
        CanvasState::SelectingEntries(selection) => {
            // highlight all of the entries
            for drawn in trip_line_buf[0..tab.cached_trips.as_ref().unwrap().len()]
                .iter()
                .filter(|it| selection.iter().any(|s| it.entity == s.parent))
            {
                let mut stroke = drawn.stroke.egui_stroke(ui.visuals().dark_mode);
                stroke.width = stroke.width + 3.0 * selection_strength;
                for (i, (line_group, entity_group)) in drawn.points[0..drawn.draw_amount]
                    .iter()
                    .zip(drawn.entries.iter())
                    .enumerate()
                {
                    let line = line_group.as_flattened().to_vec();
                    painter.line(line, stroke);
                    let nan_iter = std::iter::once(None);
                    let line_group_iter = nan_iter
                        .clone()
                        .chain(line_group.iter().map(Some))
                        .chain(nan_iter);
                    for (j, ((prev_positions, curr_positions, next_positions), entity)) in
                        line_group_iter
                            .tuple_windows()
                            .zip(entity_group.iter())
                            .enumerate()
                    {
                        const OPAQUE_RADIUS: f32 = 20.0;
                        const TRANSPARENT_RADIUS: f32 = 40.0;
                        let curr_positions = curr_positions.unwrap();
                        let midpoint = curr_positions[1].lerp(curr_positions[2], 0.5);
                        let pos = ui.input(|r| r.pointer.hover_pos());
                        let strength = if let Some(pos) = pos {
                            let len = pos.distance(midpoint);
                            let clamped = len.clamp(OPAQUE_RADIUS, TRANSPARENT_RADIUS);
                            1.0 - (clamped - OPAQUE_RADIUS) / (TRANSPARENT_RADIUS - OPAQUE_RADIUS)
                        } else {
                            0.0
                        };
                        world
                            .run_system_cached_with(
                                draw_handles,
                                (
                                    curr_positions,
                                    (
                                        *entity,
                                        drawn.entity,
                                        prev_positions.map(|it| it[3]),
                                        next_positions.map(|it| it[0]),
                                    ),
                                    (i, j),
                                    ui,
                                    &mut painter,
                                    tab.navi.zoom_x(),
                                    strength,
                                ),
                            )
                            .unwrap();
                    }
                }
            }

            // Check selection
            // if let Some(pos) = interact_pos {
            //     match (
            //         select_trip(&trip_line_buf, pos),
            //         ui.input(|r| r.modifiers.command),
            //     ) {
            //         (Some(s), true) => {
            //             world.write_message(ModifySelectedItems::Toggle(SelectedItem::Entries(s)));
            //         }
            //         (None, true) => {
            //             // do nothing
            //         }
            //         // Clear the selection
            //         (_, false) => {
            //             world.write_message(ModifySelectedItems::Clear);
            //         }
            //     };
            // }
        }
        // Select more intervals or quit the current state
        CanvasState::SelectingIntervals(i) => {
            // highlight the intervals
        }
        // Select more stations or quit the current state
        CanvasState::SelectingStations(i) => {
            // highlight the stations
        }
        // Extend the trip or quit the current state
        CanvasState::ExtendingTrip(i) => {
            // TODO: restore that
        }
    }

    // Draw time indicator
    let ticks = world.resource::<GlobalTimer>().read_ticks();
    let time_indicator_stroke = Stroke::new(1.5, Color32::RED);
    let mut time_indicator_x = tab.navi.logical_x_to_screen_x(ticks);
    time_indicator_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut time_indicator_x);

    // Draw the indicator's indicator
    display_time_indicator_indicator_horizontal(
        ui.id().with("time indicator"),
        ui.clip_rect(),
        time_indicator_x,
        time_indicator_stroke.color,
        &painter,
    );
    painter.vline(
        time_indicator_x,
        response.rect.top()..=response.rect.bottom(),
        time_indicator_stroke,
    );
}

fn draw_trip_lines<'a>(
    (
        InRef(trips),
        InMut(ui),
        InMut(painter),
        // InRef(selected),
        InMut((indices, rects)),
        In(strength),
    ): (
        InRef<[DrawnTrip]>,
        InMut<Ui>,
        InMut<Painter>,
        // InRef<[EntrySelection]>,
        InMut<(Vec<usize>, Vec<Rect>)>,
        In<f32>,
    ),
    name_q: Query<&Name, With<Trip>>,
) {
    indices.clear();
    rects.clear();

    for (idx, trip) in trips.iter().enumerate() {
        // let selected_entries: Vec<Entity> = selected
        //     .iter()
        //     .filter(|EntrySelection { parent, .. }| *parent == trip.entity)
        //     .map(|EntrySelection { entry, .. }| *entry)
        //     .collect();
        let selected_entries: &[Entity] = &[];

        if selected_entries.is_empty() {
            continue;
        }

        indices.push(idx);
        rects.extend(
            trip.points
                .iter()
                .flatten()
                .zip(trip.entries.iter().flatten())
                .filter_map(|(p, e)| {
                    if selected_entries.iter().any(|it| it == e) {
                        Some(Rect::from_two_pos(p[1], p[2]).expand(8.0))
                    } else {
                        None
                    }
                }),
        );
    }
    if strength < 0.1 {
        return;
    }
    for trip in trips.iter() {
        let draw_color = trip
            .stroke
            .color
            .get(ui.visuals().dark_mode)
            .gamma_multiply(strength);
        let name = name_q.get(trip.entity).unwrap().to_string();
        let galley = painter.layout_no_wrap(name, egui::FontId::proportional(13.0), draw_color);
        for ([.., curr], [next, ..]) in trip.points.iter().filter_map(|it| {
            if let (Some(a), Some(b)) = (it.get(0), it.get(1)) {
                return Some((a, b));
            } else {
                return None;
            }
        }) {
            let angle = (*next - *curr).angle();
            let text_shape = TextShape::new(
                *curr
                    - Vec2 {
                        y: galley.rect.height(),
                        x: 0.0,
                    },
                galley.clone(),
                draw_color,
            )
            .with_angle_and_anchor(angle, Align2::LEFT_BOTTOM);
            painter.add(text_shape);
        }
    }
}

// fn select_trip(drawn_trips: &[DrawnTrip], pos: Pos2) -> Option<EntrySelection> {
//     const VEHICLE_SELECTION_RADIUS: f32 = 7.0;
//     for trip in drawn_trips {
//         for (points, entries) in trip.points.iter().zip(trip.entries.iter()) {
//             let last = points
//                 .last()
//                 .into_iter()
//                 .flat_map(|it| {
//                     let [a, b, c, d] = it;
//                     [[*a, *b], [*b, *c], [*c, *d]]
//                 })
//                 .zip(
//                     entries
//                         .last()
//                         .into_iter()
//                         .flat_map(|it| std::iter::repeat(*it).take(3)),
//                 );
//             let entries_iter = entries
//                 .array_windows()
//                 .flat_map(|[a, b]| std::iter::repeat(*a).take(4).chain(std::iter::once(*b)));
//             for ([curr, next], e) in points
//                 .array_windows()
//                 .flat_map(|[[a1, a2, a3, a4], [b, ..]]| {
//                     let mid = a4.lerp(*b, 0.5);
//                     [[*a1, *a2], [*a2, *a3], [*a3, *a4], [*a4, mid], [mid, *b]]
//                 })
//                 .zip(entries_iter)
//                 .chain(last)
//             {
//                 let a = pos.x - curr.x;
//                 let b = pos.y - curr.y;
//                 let c = next.x - curr.x;
//                 let d = next.y - curr.y;
//                 let dot = a * c + b * d;
//                 let len_sq = c * c + d * d;
//                 if len_sq == 0.0 {
//                     continue;
//                 }
//                 let t = (dot / len_sq).clamp(0.0, 1.0);
//                 let px = curr.x + t * c;
//                 let py = curr.y + t * d;
//                 let dx = pos.x - px;
//                 let dy = pos.y - py;

//                 if dx * dx + dy * dy < VEHICLE_SELECTION_RADIUS.powi(2) {
//                     return Some(EntrySelection {
//                         entry: e,
//                         parent: trip.entity,
//                     });
//                 }
//             }
//         }
//     }
//     None
// }

fn select_station(drawn_trips: &[DrawnTrip], pos: Pos2) -> SelectedItem {
    const STATION_SELECTION_RADIUS: f32 = 7.0;
    SelectedItem::None
}

fn draw_handles(
    (
        InRef(p),
        In((e, parent_entity, prev_pos, next_pos)),
        In(salt),
        InMut(ui),
        InMut(mut painter),
        In(zoom_x),
        In(strength),
    ): (
        InRef<[Pos2]>,
        In<(Entity, Entity, Option<Pos2>, Option<Pos2>)>,
        In<impl std::hash::Hash + Copy>,
        InMut<Ui>,
        InMut<Painter>,
        In<f32>,
        In<f32>,
    ),
    entry_q: Query<EntryQuery>,
    entry_mode_q: Query<(&EntryMode, Option<&EntryEstimate>)>,
    trip_q: Query<TripQuery>,
    name_q: Query<&Name>,
    mut commands: Commands,
    mut prev_drag_delta: Local<Option<f32>>,
) {
    let entry = entry_q.get(e).unwrap();
    let trip = trip_q.get(parent_entity).unwrap();

    if entry.is_derived() || strength <= 0.1 {
        return;
    }

    // Define some sizes
    const HANDLE_SIZE: f32 = 15.0;
    const CIRCLE_HANDLE_SIZE: f32 = 7.0 / 12.0 * HANDLE_SIZE;
    const TRIANGLE_HANDLE_SIZE: f32 = 10.0 / 12.0 * HANDLE_SIZE;
    const DASH_HANDLE_SIZE: f32 = 9.0 / 12.0 * HANDLE_SIZE;

    let mut arrival_pos = p[1];
    let departure_pos: Pos2;
    if (p[1].x - p[2].x).abs() < HANDLE_SIZE {
        let midpoint_x = (p[1].x + p[2].x) / 2.0;
        arrival_pos.x = midpoint_x - HANDLE_SIZE / 2.0;
        let mut pos = p[2];
        pos.x = midpoint_x + HANDLE_SIZE / 2.0;
        departure_pos = pos;
    } else {
        departure_pos = p[2];
    }

    let handle_stroke = egui::Stroke {
        width: 2.5,
        color: Color32::BLACK.linear_multiply(strength),
    };

    let arrival_rect = Rect::from_center_size(arrival_pos, Vec2::splat(HANDLE_SIZE));
    let arrival_id = ui.id().with((e, "arr", salt));
    let arrival_response = ui.interact(arrival_rect, arrival_id, Sense::click_and_drag());

    let popup_alignment = match (prev_pos, next_pos) {
        (Some(prev), Some(next)) => {
            if prev.y >= arrival_pos.y && next.y >= arrival_pos.y {
                // Current is a local top; keep popup above both neighbors.
                RectAlign::TOP_START
            } else if prev.y <= arrival_pos.y && next.y <= arrival_pos.y {
                // Current is a local bottom; keep popup below both neighbors.
                RectAlign::BOTTOM_START
            } else if next.y >= prev.y {
                RectAlign::TOP_START
            } else {
                RectAlign::BOTTOM_START
            }
        }
        (Some(prev), None) => {
            if prev.y >= arrival_pos.y {
                RectAlign::TOP_START
            } else {
                RectAlign::BOTTOM_START
            }
        }
        (None, Some(next)) => {
            if next.y >= arrival_pos.y {
                RectAlign::TOP_START
            } else {
                RectAlign::BOTTOM_START
            }
        }
        (None, None) => RectAlign::BOTTOM_START,
    };

    arrival_popup(
        &arrival_response,
        &entry,
        &trip,
        &entry_mode_q,
        popup_alignment,
        &mut commands,
    );
    let arrival_fill = if arrival_response.hovered() {
        Color32::GRAY
    } else {
        Color32::WHITE
    }
    .linear_multiply(strength);
    match entry.mode.arr {
        Some(TravelMode::At(_)) => buttons::circle_button_shape(
            &mut painter,
            arrival_pos,
            CIRCLE_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
        Some(TravelMode::For(_)) => buttons::dash_button_shape(
            &mut painter,
            arrival_pos,
            DASH_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
        Some(TravelMode::Flexible) => buttons::triangle_button_shape(
            &mut painter,
            arrival_pos,
            TRIANGLE_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
        None => buttons::double_triangle(
            &mut painter,
            arrival_pos,
            DASH_HANDLE_SIZE,
            handle_stroke,
            arrival_fill,
        ),
    };

    if arrival_response.drag_started() {
        *prev_drag_delta = None;
    }
    if let Some(total_drag_delta) = arrival_response.total_drag_delta() {
        if zoom_x > f32::EPSILON {
            let previous_drag_delta = prev_drag_delta.unwrap_or(0.0);
            let delta_ticks = Tick(
                ((total_drag_delta.x as f64 - previous_drag_delta as f64) / zoom_x as f64) as i64,
            );
            let duration = Duration(delta_ticks.to_timetable_time().0);
            if duration != Duration(0) {
                commands.trigger(AdjustEntryMode {
                    entity: e,
                    adj: EntryModeAdjustment::ShiftArrival(duration),
                });
                let consumed_ticks = Tick::from_timetable_time(TimetableTime(duration.0));
                *prev_drag_delta =
                    Some(previous_drag_delta + (consumed_ticks.0 as f64 * zoom_x as f64) as f32);
            }
        }
    }
    if arrival_response.drag_stopped() {
        *prev_drag_delta = None;
    }
    if arrival_response.dragged() || arrival_response.hovered() {
        arrival_response.on_hover_ui(|ui| {
            if let Some(estimate) = entry.estimate {
                ui.label(estimate.arr.to_string());
            }
            ui.label(name_q.get(entry.stop()).map_or("??", |n| n.as_str()));
        });
    }

    let dep_sense = match entry.mode.dep {
        TravelMode::Flexible => Sense::click(),
        _ => Sense::click_and_drag(),
    };
    let departure_rect = Rect::from_center_size(departure_pos, Vec2::splat(HANDLE_SIZE));
    let departure_id = ui.id().with((e, "dep", salt));
    let departure_response = ui.interact(departure_rect, departure_id, dep_sense);
    departure_popup(&departure_response, &entry, popup_alignment, &mut commands);
    let departure_fill = if departure_response.hovered() {
        Color32::GRAY
    } else {
        Color32::WHITE
    }
    .linear_multiply(strength);
    match entry.mode.dep {
        TravelMode::At(_) => buttons::circle_button_shape(
            &mut painter,
            departure_pos,
            CIRCLE_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
        TravelMode::For(_) => buttons::dash_button_shape(
            &mut painter,
            departure_pos,
            DASH_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
        TravelMode::Flexible => buttons::triangle_button_shape(
            &mut painter,
            departure_pos,
            TRIANGLE_HANDLE_SIZE,
            handle_stroke,
            departure_fill,
        ),
    };

    if departure_response.drag_started() {
        *prev_drag_delta = None;
    }
    if let Some(total_drag_delta) = departure_response.total_drag_delta() {
        if zoom_x > f32::EPSILON {
            let previous_drag_delta = prev_drag_delta.unwrap_or(0.0);
            let delta_ticks = Tick(
                ((total_drag_delta.x as f64 - previous_drag_delta as f64) / zoom_x as f64) as i64,
            );
            let duration = Duration(delta_ticks.to_timetable_time().0);
            if duration != Duration(0) {
                commands.trigger(AdjustEntryMode {
                    entity: e,
                    adj: EntryModeAdjustment::ShiftDeparture(duration),
                });
                let consumed_ticks = Tick::from_timetable_time(TimetableTime(duration.0));
                *prev_drag_delta =
                    Some(previous_drag_delta + (consumed_ticks.0 as f64 * zoom_x as f64) as f32);
            }
        }
    }
    if departure_response.drag_stopped() {
        *prev_drag_delta = None;
    }
    if departure_response.dragged() || departure_response.hovered() {
        departure_response.on_hover_ui(|ui| {
            if let Some(estimate) = entry.estimate {
                ui.label(estimate.dep.to_string());
            }
            ui.label(name_q.get(entry.stop()).map_or("??", |n| n.as_str()));
        });
    }
}
