// SPDX-License-Identifier: MPL-2.0

use std::borrow::Cow;
use std::io;

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, LineCap, LineJoin, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, TextDirection};
use krilla::{Data, Document};
use paiagram_core::route::{DiagramCache, RouteInterval, StationRecord};
use paiagram_core::time::Tick;
use paiagram_core::{CanvasLength, RouteKey, WorldSnapshot};

/// 1 PDF point is 1/72 inch, whereas [`CanvasLength`] is measured in millimetres.
const PT_PER_MM: f64 = 72.0 / 25.4;
/// Horizontal scale: five minutes of timetable span one centimetre of canvas, matching the
/// diagram tab's default zoom.
const TICKS_PER_5_MIN: f64 = 5.0 * 60.0 * 100.0;
/// Padding (in PDF points) around the diagram.
const MARGIN: f64 = 20.0;
/// Extra time shown on either side of the diagram, in ticks (ten minutes).
const TIME_PADDING_TICKS: i64 = 10 * 60 * 100;
/// The largest allowed PDF page dimension (200 inches).
const MAX_PAGE: f64 = 14400.0;
/// Text size in PDF points; 13 egui points (1/96 inch) converted to 1/72 inch.
const TEXT_SIZE: f32 = 13.0 * 72.0 / 96.0;

/// Candidate time-grid spacings, in seconds.
const TIME_SPACINGS_S: [i64; 10] = [1, 10, 30, 60, 300, 600, 1800, 3600, 14400, 86400];
const TIME_LINE_MIN_WIDTH: f64 = 32.0;
const TIME_LINE_MAX_WIDTH: f64 = 64.0;

#[derive(Clone, Copy)]
pub enum GraphyFormat {
    Pdf,
    Svg,
    Png,
}

#[derive(Clone)]
pub struct ExportGraphy {
    pub snap: WorldSnapshot,
    pub route: RouteKey,
    pub cache: DiagramCache,
    pub format: GraphyFormat,
    pub fonts: Vec<Cow<'static, [u8]>>,
}

impl paiagram_rw::ExportObject for ExportGraphy {
    fn write_content<W: io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        match self.format {
            GraphyFormat::Pdf => {
                let font = self
                    .fonts
                    .first()
                    .and_then(|data| Font::new(krilla_data(data), 0))
                    .ok_or_else(|| io::Error::other("no usable font was provided"))?;
                let document = new_document(&self.snap, self.route, &self.cache, font)
                    .ok_or_else(|| io::Error::other("failed to build the PDF document"))?;
                writer.write_all(&document)
            }
            GraphyFormat::Svg | GraphyFormat::Png => {
                todo!()
            }
        }
    }
    fn extension(&self) -> impl AsRef<str> {
        match self.format {
            GraphyFormat::Pdf => ".pdf",
            GraphyFormat::Svg => ".svg",
            GraphyFormat::Png => ".png",
        }
    }
}

fn krilla_data(data: &Cow<'static, [u8]>) -> Data {
    match data {
        Cow::Borrowed(bytes) => Data::from(*bytes),
        Cow::Owned(bytes) => Data::from(bytes.clone()),
    }
}

fn new_document(
    snap: &WorldSnapshot,
    route_key: RouteKey,
    cache: &DiagramCache,
    font: Font,
) -> Option<Vec<u8>> {
    let route = snap.routes.get(&route_key)?;
    let (station_heights, station_names, total_height_mm) = station_lines(snap, &route.intervals.0);

    // Time span covered by the diagram, padded a little on both sides. When the cache is empty,
    // fall back to a whole day so there is still something to look at.
    let (mut t_min, mut t_max) = time_range(cache).unwrap_or((0, 24 * 3600 * 100));
    t_min -= TIME_PADDING_TICKS;
    t_max += TIME_PADDING_TICKS;
    let time_span = (t_max - t_min).max(1) as f64;

    // Five minutes per centimetre horizontally, and 1:1 vertically (so one millimetre of canvas
    // is one millimetre on the page).
    let mut x_per_tick = CanvasLength::from_cm(1.0).to_postscript_pts() / TICKS_PER_5_MIN;
    let mut y_per_mm = PT_PER_MM;

    // Shrink uniformly if the drawing would exceed the maximum PDF page size.
    let shrink = {
        let width = time_span * x_per_tick + MARGIN * 2.0;
        let height = total_height_mm * y_per_mm + MARGIN * 2.0;
        (MAX_PAGE / width).min(MAX_PAGE / height).min(1.0)
    };
    x_per_tick *= shrink;
    y_per_mm *= shrink;

    let width = (time_span * x_per_tick + MARGIN * 2.0) as f32;
    let height = (total_height_mm * y_per_mm + MARGIN * 2.0) as f32;

    let to_x = |tick: i64| (MARGIN + (tick - t_min) as f64 * x_per_tick) as f32;
    let to_y = |mm: f64| (MARGIN + mm * y_per_mm) as f32;

    let content_left = to_x(t_min);
    let content_right = to_x(t_max);
    let content_top = to_y(0.0);
    let content_bottom = to_y(total_height_mm);

    let mut document = Document::new();
    document.set_metadata(
        Metadata::new()
            .creator(concat!("Paiagram ", env!("CARGO_PKG_VERSION")).into())
            .keywords(vec![
                "Timetable".into(),
                "Paiagram".into(),
                "Train".into(),
                "Transport".into(),
                "Marey Chart".into(),
            ])
            .title("Paiagram Timetable".into()),
    );
    let mut page = document.start_page_with(PageSettings::from_wh(width, height)?);
    let mut surface = page.surface();

    // White background.
    let background = {
        let mut pb = PathBuilder::new();
        pb.push_rect(Rect::from_xywh(0.0, 0.0, width, height)?);
        pb.finish()?
    };
    surface.set_stroke(None);
    surface.set_fill(Some(Fill {
        paint: rgb::Color::new(255, 255, 255).into(),
        opacity: NormalizedF32::ONE,
        rule: Default::default(),
    }));
    surface.draw_path(&background);

    // Station lines.
    let station_stroke = gray_stroke(0.6, 1.0);
    for height_mm in &station_heights {
        let y = to_y(height_mm.0);
        draw_line(
            &mut surface,
            content_left,
            y,
            content_right,
            y,
            &station_stroke,
        );
    }

    // Time scale (lines and labels).
    draw_time_lines(
        &mut surface,
        &font,
        t_min,
        t_max,
        content_top,
        content_bottom,
        x_per_tick,
        to_x,
    );

    // Trip (diagram) lines.
    draw_trips(snap, cache, &station_heights, &mut surface, to_x, to_y);

    // Station labels, drawn last so they stay legible on top of the diagram lines.
    if station_names.iter().any(|name| !name.is_empty()) {
        surface.set_stroke(None);
        surface.set_fill(Some(gray_fill(1.0)));
        for (name, height_mm) in station_names.iter().zip(&station_heights) {
            if name.is_empty() {
                continue;
            }
            draw_text(
                &mut surface,
                &font,
                content_left + 2.0,
                to_y(height_mm.0) - 2.0,
                name,
                false,
            );
        }
    }

    surface.finish();
    page.finish();
    document.finish().ok()
}

/// The vertical position (in canvas millimetres) of the start of every route interval, the name
/// of the station at that position, and the total height of the diagram.
fn station_lines(
    snap: &WorldSnapshot,
    intervals: &[RouteInterval],
) -> (Vec<CanvasLength>, Vec<String>, f64) {
    let mut heights = Vec::with_capacity(intervals.len());
    let mut names = Vec::with_capacity(intervals.len());
    let mut total = 0.0_f64;
    for interval in intervals {
        heights.push(CanvasLength(total));
        names.push(station_name(snap, &interval.station_record).unwrap_or_default());
        total += interval.canvas_length.unwrap_or_else(|| CanvasLength::from_cm(1.5)).0;
    }
    (heights, names, total)
}

/// The name of the station an interval's [`StationRecord`] belongs to.
fn station_name(snap: &WorldSnapshot, record: &StationRecord) -> Option<String> {
    let station = match record {
        StationRecord::All(key) => *key,
        StationRecord::Some(nodes) => {
            let node = nodes.first()?;
            snap.graph.nodes().get(node)?.parent
        }
    };
    snap.stations.get(&station).map(|station| station.name.to_string())
}

/// The earliest and latest time (in ticks) covered by the cached trip polylines.
fn time_range(cache: &DiagramCache) -> Option<(i64, i64)> {
    let mut min = i64::MAX;
    let mut max = i64::MIN;
    for polylines in cache.map.values() {
        for polyline in polylines {
            for (estimate, _entry, _idx, _progress) in polyline {
                let arr = estimate.arr.to_ticks().0;
                let dep = estimate.dep.to_ticks().0;
                min = min.min(arr).min(dep);
                max = max.max(arr).max(dep);
            }
        }
    }
    (min <= max).then_some((min, max))
}

fn draw_time_lines(
    surface: &mut Surface<'_>,
    font: &Font,
    t_min: i64,
    t_max: i64,
    top: f32,
    bottom: f32,
    x_per_tick: f64,
    to_x: impl Fn(i64) -> f32,
) {
    // How many ticks a single PDF point spans.
    let ticks_per_point = 1.0 / x_per_tick;
    let sizes: Vec<i64> = TIME_SPACINGS_S.iter().map(|secs| secs * 100).collect();
    let first = sizes
        .iter()
        .position(|spacing| *spacing as f64 / ticks_per_point * 1.5 > TIME_LINE_MIN_WIDTH)
        .unwrap_or(0);

    let mut drawn: Vec<i64> = Vec::new();
    // Coarsest spacing first, so finer grids skip the ticks that are already drawn.
    for (level, &spacing) in sizes.iter().enumerate().skip(first).rev() {
        let strength = ((spacing as f64 / ticks_per_point * 1.5 - TIME_LINE_MIN_WIDTH)
            / (TIME_LINE_MAX_WIDTH - TIME_LINE_MIN_WIDTH))
            .clamp(0.0, 1.0);
        if strength < 0.1 {
            continue;
        }
        let stroke = gray_stroke(0.5, strength as f32);
        let mut tick = t_min - t_min.rem_euclid(spacing) - spacing;
        while tick <= t_max {
            tick += spacing;
            if drawn.contains(&tick) {
                continue;
            }
            drawn.push(tick);
            let x = to_x(tick);
            draw_line(surface, x, top, x, bottom, &stroke);
            let label = time_label(Tick(tick).to_timetable_time(), level);
            surface.set_stroke(None);
            surface.set_fill(Some(gray_fill(1.0)));
            draw_text(surface, font, x, top - 3.0, &label, true);
        }
    }
}

/// The label of a time-scale tick, matching the diagram tab: seconds for the finest levels,
/// `H:MM` for the minute levels and the full `HH:MM:SS` for the day level.
fn time_label(time: paiagram_core::time::TimetableTime, level: usize) -> String {
    let (hours, minutes, seconds, _) = time.to_hmsd();
    match level {
        0..=2 => seconds.to_string(),
        3..=8 => format!("{hours}:{minutes:02}"),
        _ => time.to_string(),
    }
}

fn draw_trips(
    snap: &WorldSnapshot,
    cache: &DiagramCache,
    station_heights: &[CanvasLength],
    surface: &mut Surface<'_>,
    to_x: impl Fn(i64) -> f32,
    to_y: impl Fn(f64) -> f32,
) {
    for (trip_key, polylines) in &cache.map {
        let Some(trip) = snap.trips.get(trip_key) else {
            continue;
        };
        let stroke = trip.service_class.and_then(|key| snap.service_classes.get(&key)).map_or_else(
            || gray_stroke(1.0, 1.0),
            |class| {
                let [r, g, b, a] = class.style.color.to_array();
                Stroke {
                    paint: rgb::Color::new(r, g, b).into(),
                    width: class.style.width as f32,
                    opacity: NormalizedF32::new(a as f32 / 255.0).unwrap_or(NormalizedF32::ONE),
                    ..Stroke::default()
                }
            },
        );

        for polyline in polylines {
            // Each entry spans `arr`..`dep` at one placement, so it contributes two points at
            // the same height (mirroring the diagram tab's CPU drawing).
            let mut points = Vec::with_capacity(polyline.len() * 2);
            for (estimate, _entry, idx, progress) in polyline {
                let (Some(start), Some(end)) = (
                    station_heights.get(*idx as usize),
                    station_heights.get(*idx as usize + 1),
                ) else {
                    continue;
                };
                let y_mm = start.0 + (end.0 - start.0) * *progress as f64;
                let y = to_y(y_mm);
                points.push((to_x(estimate.arr.to_ticks().0), y));
                points.push((to_x(estimate.dep.to_ticks().0), y));
            }
            draw_polyline(surface, &points, &stroke);
        }
    }
}

fn gray_fill(opacity: f32) -> Fill {
    Fill {
        paint: rgb::Color::new(128, 128, 128).into(),
        opacity: NormalizedF32::new(opacity).unwrap_or(NormalizedF32::ONE),
        rule: Default::default(),
    }
}

fn gray_stroke(width: f32, opacity: f32) -> Stroke {
    Stroke {
        paint: rgb::Color::new(128, 128, 128).into(),
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        opacity: NormalizedF32::new(opacity).unwrap_or(NormalizedF32::ONE),
        ..Stroke::default()
    }
}

fn draw_line(surface: &mut Surface<'_>, x1: f32, y1: f32, x2: f32, y2: f32, stroke: &Stroke) {
    draw_polyline(surface, &[(x1, y1), (x2, y2)], stroke);
}

fn draw_polyline(surface: &mut Surface<'_>, points: &[(f32, f32)], stroke: &Stroke) {
    if points.len() < 2 {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        pb.line_to(x, y);
    }
    if let Some(path) = pb.finish() {
        surface.set_fill(None);
        surface.set_stroke(Some(stroke.clone()));
        surface.draw_path(&path);
    }
}

/// Draws a single line of text using the current fill. `y` is the baseline, and `centered`
/// centres the text horizontally on `x`.
fn draw_text(surface: &mut Surface<'_>, font: &Font, x: f32, y: f32, text: &str, centered: bool) {
    if text.is_empty() {
        return;
    }
    let x = if centered {
        x - estimate_text_width(text) * 0.5
    } else {
        x
    };
    surface.draw_text(
        Point::from_xy(x, y),
        font.clone(),
        TEXT_SIZE,
        text,
        false,
        TextDirection::Auto,
    );
}

/// A rough advance-width estimate for a line of text, in PDF points.
///
/// krilla does not expose text measurement, so we approximate the width (half an em per ASCII
/// character, a full em otherwise) to centre the time-scale labels.
fn estimate_text_width(text: &str) -> f32 {
    text.chars().map(|c| if c.is_ascii() { 0.5 } else { 1.0 }).sum::<f32>() * TEXT_SIZE
}
