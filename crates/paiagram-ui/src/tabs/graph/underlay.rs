use bevy::ecs::system::{In, InMut, InRef, Local};
use egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, Ui, Widget,
    pos2,
};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use walkers::mercator;
use walkers::sources::{Attribution, OpenStreetMap, TileSource};
use walkers::{HttpTiles, Tile, TileId, Tiles};

use crate::tabs::Navigatable;
use paiagram_core::graph::{lon_lat_to_xy, xy_to_lon_lat};

#[derive(Default, Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub enum UnderlayTileType {
    None,
    #[default]
    OpenStreetMap,
    ChiriinChizu,
    AutoNavi,
}

impl Widget for &mut UnderlayTileType {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut changed = false;
        changed |= ui
            .radio_value(self, UnderlayTileType::None, tr!("tab-graph-underlay-none"))
            .changed();
        changed |= ui
            .radio_value(
                self,
                UnderlayTileType::OpenStreetMap,
                tr!("tab-graph-underlay-openstreetmap"),
            )
            .changed();
        changed |= ui
            .radio_value(
                self,
                UnderlayTileType::AutoNavi,
                tr!("tab-graph-underlay-amap"),
            )
            .changed();
        let mut res = ui.radio_value(
            self,
            UnderlayTileType::ChiriinChizu,
            tr!("tab-graph-underlay-chiriin"),
        );
        if changed {
            res.mark_changed();
        }
        res
    }
}

pub struct ChiriinChizu;

impl TileSource for ChiriinChizu {
    fn tile_url(&self, tile_id: TileId) -> String {
        let z = tile_id.zoom;
        let x = tile_id.x;
        let y = tile_id.y;
        let id = "std";
        format!("https://cyberjapandata.gsi.go.jp/xyz/{id}/{z}/{x}/{y}.png")
    }
    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Chiri-in Chizu (Cyber Japan Data)",
            url: "https://cyberjapandata.gsi.go.jp",
            logo_dark: None,
            logo_light: None,
        }
    }
}

pub struct AutoNavi;

impl TileSource for AutoNavi {
    fn tile_url(&self, tile_id: TileId) -> String {
        let z = tile_id.zoom;
        let x = tile_id.x;
        let y = tile_id.y;

        // Cycle through subdomains 01-04 based on the x coordinate
        let subdomain = (x % 4) + 1;

        // style=8 is the standard vector road map
        format!(
            "https://webrd0{subdomain}.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}"
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "AutoNavi (Amap)",
            url: "https://www.amap.com/",
            logo_dark: None,
            logo_light: None,
        }
    }
}

pub fn draw_underlay(
    (InMut(painter), InRef(navi), InMut(ui), In(new_type)): (
        InMut<Painter>,
        InRef<super::GraphNavigation>,
        InMut<Ui>,
        In<Option<UnderlayTileType>>,
    ),
    mut visited: Local<HashSet<TileId>>,
    mut stack: Local<Vec<TileId>>,
    mut tiles: Local<Option<Option<HttpTiles>>>,
) -> Option<Attribution> {
    let text_color = painter.ctx().style().visuals.text_color();

    draw_world_grid(
        &painter,
        navi.visible_rect(),
        navi.offset_x() as f32,
        navi.offset_y() as f32,
        navi.zoom_x(),
    );

    let tiles = tiles.get_or_insert(None);

    let ctx = ui.ctx().clone();

    match new_type {
        None => {}
        Some(UnderlayTileType::None) => *tiles = None,
        Some(UnderlayTileType::OpenStreetMap) => *tiles = Some(HttpTiles::new(OpenStreetMap, ctx)),
        Some(UnderlayTileType::ChiriinChizu) => *tiles = Some(HttpTiles::new(ChiriinChizu, ctx)),
        Some(UnderlayTileType::AutoNavi) => *tiles = Some(HttpTiles::new(AutoNavi, ctx)),
    }

    let Some(tiles) = tiles else {
        return None;
    };

    let graph_zoom = navi.zoom_x() as f64;
    let tile_zoom = graph_zoom_to_tile_zoom(graph_zoom);

    let view = navi.visible_rect();

    let center_screen = view.center();
    let (center_x, center_y) = navi.screen_pos_to_xy(center_screen);
    let (center_lon, center_lat) = xy_to_lon_lat(center_x, center_y);
    let map_center = walkers::lon_lat(center_lon, center_lat);
    let map_center_projected = mercator::project(map_center, tile_zoom);

    visited.clear();
    stack.clear();
    stack.push(root_tile_id(
        map_center_projected.x(),
        map_center_projected.y(),
        tile_zoom,
        tiles.tile_size(),
    ));
    let corrected_tile_size = corrected_tile_size(tiles.tile_size(), tile_zoom);
    let clip = painter.clip_rect();

    while let Some(tile_id) = stack.pop() {
        if !visited.insert(tile_id) {
            continue;
        }

        let tile_rect = tile_rect(
            tile_id,
            corrected_tile_size,
            map_center_projected.x(),
            map_center_projected.y(),
            clip,
        );
        if !clip.intersects(tile_rect) {
            continue;
        }

        if let Some(tile_piece) = tiles.at(tile_id) {
            draw_tile_piece(&painter, tile_piece, tile_rect);
        }

        for next in [
            tile_id.north(),
            tile_id.east(),
            tile_id.south(),
            tile_id.west(),
        ]
        .into_iter()
        .flatten()
        {
            if !visited.contains(&next) {
                stack.push(next);
            }
        }
    }
    Some(tiles.attribution())
}

fn draw_world_grid(painter: &Painter, viewport: Rect, offset_x: f32, offset_y: f32, zoom: f32) {
    if zoom <= 0.0 {
        return;
    }

    const MIN_WIDTH: f32 = 32.0;
    const MAX_WIDTH: f32 = 120.0;
    let base_color = egui::Color32::from_gray(160);

    for p in ((-5)..=5).rev() {
        let spacing = 10.0f32.powi(p);
        let screen_spacing = spacing * zoom;
        let strength =
            ((screen_spacing * 1.5 - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)).clamp(0.0, 1.0);
        if strength <= 0.0 {
            continue;
        }

        let stroke = Stroke::new(0.6, base_color.gamma_multiply(strength));

        let mut n = (offset_x / spacing).floor();
        loop {
            let world_x = n * spacing;
            let screen_x_rel = (world_x - offset_x) * zoom;
            if screen_x_rel > viewport.width() {
                break;
            }
            if screen_x_rel >= 0.0 {
                painter.vline(viewport.left() + screen_x_rel, viewport.y_range(), stroke);
            }
            n += 1.0;
        }

        let mut m = (offset_y / spacing).floor();
        loop {
            let world_y = m * spacing;
            let screen_y_rel = (world_y - offset_y) * zoom;
            if screen_y_rel > viewport.height() {
                break;
            }
            if screen_y_rel >= 0.0 {
                painter.hline(viewport.x_range(), viewport.top() + screen_y_rel, stroke);
            }
            m += 1.0;
        }
    }
}

fn graph_zoom_to_tile_zoom(graph_zoom: f64) -> f64 {
    const TILE_SIZE: f64 = 256.0;
    let (x0, _) = lon_lat_to_xy(-180.0, 0.0);
    let (x1, _) = lon_lat_to_xy(180.0, 0.0);
    let world_meters = (x1 - x0).abs();
    (graph_zoom * world_meters / TILE_SIZE)
        .log2()
        .clamp(0.0, 26.0)
}

fn corrected_tile_size(source_tile_size: u32, zoom: f64) -> f64 {
    source_tile_size as f64 * 2f64.powf(zoom - zoom.round())
}

fn root_tile_id(center_px_x: f64, center_px_y: f64, zoom: f64, source_tile_size: u32) -> TileId {
    let rounded_zoom = zoom.round().clamp(0.0, 26.0) as i32;
    let zoom_offset = ((source_tile_size as f64) / 256.0).log2() as i32;
    let tile_zoom = (rounded_zoom - zoom_offset).clamp(0, 26) as u8;
    let tile_size_px = corrected_tile_size(source_tile_size, zoom);
    let max_index = (2u32.pow(tile_zoom as u32).saturating_sub(1)) as i64;

    let x = ((center_px_x / tile_size_px).floor() as i64).clamp(0, max_index) as u32;
    let y = ((center_px_y / tile_size_px).floor() as i64).clamp(0, max_index) as u32;

    TileId {
        x,
        y,
        zoom: tile_zoom,
    }
}

fn tile_rect(
    tile_id: TileId,
    corrected_tile_size: f64,
    center_px_x: f64,
    center_px_y: f64,
    clip: Rect,
) -> Rect {
    let tile_px_x = tile_id.x as f64 * corrected_tile_size;
    let tile_px_y = tile_id.y as f64 * corrected_tile_size;

    let screen_x = clip.center().x as f64 + (tile_px_x - center_px_x);
    let screen_y = clip.center().y as f64 + (tile_px_y - center_px_y);

    Rect::from_min_max(
        pos2(screen_x as f32, screen_y as f32),
        pos2(
            (screen_x + corrected_tile_size) as f32,
            (screen_y + corrected_tile_size) as f32,
        ),
    )
}

// TODO: handle vector pieces?
fn draw_tile_piece(painter: &Painter, tile_piece: walkers::TilePiece, rect: Rect) {
    match tile_piece.tile {
        Tile::Raster(texture_handle) => {
            let mut mesh = Mesh::with_texture(texture_handle.id());
            mesh.add_rect_with_uv(rect, tile_piece.uv, egui::Color32::WHITE);
            painter.add(Shape::mesh(mesh));
        }
        _ => {}
    }
}
