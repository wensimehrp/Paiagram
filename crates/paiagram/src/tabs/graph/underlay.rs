//! XYZ raster underlays using the same egui version and image loaders as the app.
use std::collections::VecDeque;

use egui::{Painter, Rect, Stroke, Ui};
use serde::{Deserialize, Serialize};

use crate::tabs::Navigatable;

#[derive(Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) enum UnderlayTileType {
    #[default]
    None,
    OpenStreetMap,
    ChiriinChizu(ChiriinChizuVariant),
    AutoNavi,
    EsriWorldImagery,
}
#[derive(Default, Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub(crate) enum ChiriinChizuVariant {
    #[default]
    Standard,
    Light,
    White,
    English,
    Satellite,
}
pub(super) struct Attribution {
    pub text: &'static str,
    pub url: &'static str,
}
impl egui::Widget for &mut UnderlayTileType {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        ui.vertical(|ui| {
            ui.radio_value(self, UnderlayTileType::None, "None");
            ui.radio_value(self, UnderlayTileType::OpenStreetMap, "OpenStreetMap");
            ui.radio_value(self, UnderlayTileType::EsriWorldImagery, "Esri imagery");
            ui.radio_value(self, UnderlayTileType::AutoNavi, "AutoNavi");
            ui.collapsing("Japan GSI", |ui| {
                for variant in [
                    ChiriinChizuVariant::Standard,
                    ChiriinChizuVariant::Light,
                    ChiriinChizuVariant::White,
                    ChiriinChizuVariant::English,
                    ChiriinChizuVariant::Satellite,
                ] {
                    ui.radio_value(
                        self,
                        UnderlayTileType::ChiriinChizu(variant),
                        format!("{variant:?}"),
                    );
                }
            });
        })
        .response
    }
}
impl UnderlayTileType {
    fn url(self, z: i32, x: i32, y: i32) -> String {
        match self {
            Self::None => String::new(),
            Self::OpenStreetMap => format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"),
            Self::EsriWorldImagery => format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
            ),
            Self::AutoNavi => format!(
                "https://webrd01.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}"
            ),
            Self::ChiriinChizu(variant) => {
                let (id, ext) = match variant {
                    ChiriinChizuVariant::Standard => ("std", "png"),
                    ChiriinChizuVariant::Light => ("pale", "png"),
                    ChiriinChizuVariant::White => ("blank", "png"),
                    ChiriinChizuVariant::English => ("english", "png"),
                    ChiriinChizuVariant::Satellite => ("seamlessphoto", "jpg"),
                };
                format!("https://cyberjapandata.gsi.go.jp/xyz/{id}/{z}/{x}/{y}.{ext}")
            }
        }
    }
    fn attribution(self) -> Option<Attribution> {
        Some(match self {
            Self::None => return None,
            Self::OpenStreetMap => Attribution {
                text: "OpenStreetMap contributors",
                url: "https://www.openstreetmap.org/copyright",
            },
            Self::EsriWorldImagery => Attribution {
                text: "Esri",
                url: "https://developers.arcgis.com/documentation/esri-and-data-attribution/",
            },
            Self::AutoNavi => Attribution {
                text: "AutoNavi (Amap)",
                url: "https://www.amap.com/",
            },
            Self::ChiriinChizu(_) => Attribution {
                text: "GSI Japan",
                url: "https://cyberjapandata.gsi.go.jp/",
            },
        })
    }
}
#[derive(Default)]
pub(super) struct UnderlayPainter {
    tile_type: UnderlayTileType,
    recent: VecDeque<String>,
}
impl UnderlayPainter {
    pub fn update_tile_type(&mut self, new_type: Option<UnderlayTileType>) {
        if let Some(t) = new_type {
            self.tile_type = t;
        }
    }
    pub fn draw_underlay(
        &mut self,
        painter: &mut Painter,
        navi: &super::GraphNavigation,
        ui: &mut Ui,
    ) -> Option<Attribution> {
        let view = navi.visible_rect();
        let zoom = navi.zoom_x() as f64;
        let spacing = 10.0_f64.powf((80.0 / zoom).log10().round());
        let stroke = Stroke::new(0.5, ui.visuals().weak_text_color().gamma_multiply(0.25));
        for i in 0..=(view.width() as f64 / (spacing * zoom)).ceil() as usize + 1 {
            let x = ((navi.offset_x() / spacing).floor() + i as f64) * spacing;
            painter.vline(navi.logical_x_to_screen_x(x), view.y_range(), stroke);
        }
        for i in 0..=(view.height() as f64 / (spacing * zoom)).ceil() as usize + 1 {
            let y = ((navi.offset_y() / spacing).floor() + i as f64) * spacing;
            painter.hline(view.x_range(), navi.logical_y_to_screen_y(y), stroke);
        }
        let attribution = self.tile_type.attribution()?;
        egui_extras::install_image_loaders(ui.ctx());
        const WORLD: f64 = 40_075_016.685_578_49;
        let z = (zoom * WORLD / 256.0).log2().round().clamp(0.0, 19.0) as i32;
        let count = 1_i32 << z;
        let tile_metres = WORLD / count as f64;
        let xr = navi.visible_x();
        let yr = navi.visible_y();
        let index = |v: f64| ((v + WORLD / 2.0) / tile_metres).floor() as i32;
        let clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(clip));
        for x in index(xr.start).max(0)..=index(xr.end).min(count - 1) {
            for y in index(yr.start).max(0)..=index(yr.end).min(count - 1) {
                let url = self.tile_type.url(z, x, y);
                let min = navi.xy_to_screen_pos(
                    x as f64 * tile_metres - WORLD / 2.0,
                    y as f64 * tile_metres - WORLD / 2.0,
                );
                let rect = Rect::from_min_size(min, egui::Vec2::splat((tile_metres * zoom) as f32));
                egui::Image::new(url.clone()).show_loading_spinner(false).paint_at(ui, rect);
                if let Some(i) = self.recent.iter().position(|u| u == &url) {
                    self.recent.remove(i);
                }
                self.recent.push_back(url);
            }
        }
        ui.set_clip_rect(clip);
        while self.recent.len() > 256 {
            if let Some(url) = self.recent.pop_front() {
                ui.ctx().forget_image(&url);
            }
        }
        Some(attribution)
    }
}
