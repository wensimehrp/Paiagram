//! XYZ raster underlays using the same egui version and image loaders as the app.
use std::collections::VecDeque;

use egui::{ComboBox, Painter, Rect, Stroke, Ui};
use serde::{Deserialize, Serialize};

use crate::tabs::Navigatable;

macro_rules! named_enum {
    (@bind $ty:ty, $id:ident) => { $id };
    {
        $(#[$ea:meta])* $vis:vis enum $Enum:ident;
        $($(#[$va:meta])* $Variant:ident $(($Data:ty))? => {
            name: $name:expr,
            attr_text: $text:expr,
            attr_url: $url:expr,
            tile_url: $tile:expr,
        },)*
    } => {
        $(#[$ea])*
        $vis enum $Enum {
            $($(#[$va])*
            $Variant $(($Data))?,)*
        }

        impl $Enum {
            pub fn all() -> impl Iterator<Item = Self> {
                [$(Self::$Variant $((<$Data>::default()))?,)*].into_iter()
            }
            pub fn name(&self) -> &'static str {
                match self { $(
                    Self::$Variant $((named_enum!(@bind $Data, _v)))? => $name,
                )* }
            }
            pub fn attr(&self) -> (&'static str, &'static str) {
                match self { $(
                    Self::$Variant $((named_enum!(@bind $Data, _v)))? => ($text, $url),
                )* }
            }
            pub fn tile_url(&self, z: i32, x: i32, y: i32) -> Option<String> {
                match self { $(
                    Self::$Variant $((named_enum!(@bind $Data, v)))? =>
                        ($tile)(z, x, y $(, named_enum!(@bind $Data, v))?),
                )* }
            }
        }
    };
}

named_enum! {
    #[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
    pub(crate) enum UnderlayTileType;
    #[default]
    None => {
        name: "None",
        attr_text: "",
        attr_url: "",
        tile_url: |_, _, _| None,
    },
    OpenStreetMap => {
        name: "OpenStreetMap",
        attr_text: "OpenStreetMap contributors",
        attr_url: "https://www.openstreetmap.org/copyright",
        tile_url: |z, x, y| Some(format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png")),
    },
    ChiriinChizu(ChiriinChizuVariant) => {
        name: "Chiri-in Chizu",
        attr_text: "GSI Japan",
        attr_url: "https://cyberjapandata.gsi.go.jp/",
        tile_url: |z, x, y, v: &ChiriinChizuVariant| {
            let (id, ext) = match *v {
                ChiriinChizuVariant::Standard => ("std", "png"),
                ChiriinChizuVariant::Light => ("pale", "png"),
                ChiriinChizuVariant::White => ("blank", "png"),
                ChiriinChizuVariant::English => ("english", "png"),
                ChiriinChizuVariant::Satellite => ("seamlessphoto", "jpg"),
            };
            Some(format!("https://cyberjapandata.gsi.go.jp/xyz/{id}/{z}/{x}/{y}.{ext}"))
        },
    },
    AutoNavi => {
        name: "AutoNavi (Gaode)",
        attr_text: "AutoNavi (Amap)",
        attr_url: "https://www.amap.com/",
        tile_url: |z, x, y| Some(format!(
            "https://webrd01.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=8&x={x}&y={y}&z={z}"
        )),
    },
    EsriWorldImagery => {
        name: "ESRI World Imagery",
        attr_text: "Esri",
        attr_url: "https://developers.arcgis.com/documentation/esri-and-data-attribution/",
        tile_url: |z, x, y| Some(format!(
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
        )),
    },
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Debug)]
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
            for val in UnderlayTileType::all() {
                if let UnderlayTileType::ChiriinChizu(..) = val {
                    ui.horizontal(|ui| {
                        ui.radio_value(self, val, val.name());
                        if let UnderlayTileType::ChiriinChizu(variant) = self {
                            ComboBox::from_id_salt("glsjfksdfjlks")
                                .selected_text(format!("{:?}", variant))
                                .show_ui(ui, |ui| {
                                    use ChiriinChizuVariant::*;
                                    ui.radio_value(variant, Standard, "Standard");
                                    ui.radio_value(variant, Light, "Light");
                                    ui.radio_value(variant, White, "White");
                                    ui.radio_value(variant, English, "English");
                                    ui.radio_value(variant, Satellite, "Satellite");
                                });
                        } else {
                            ui.add_enabled_ui(false, |ui| {
                                egui::ComboBox::from_id_salt("balabala")
                                    .selected_text("Standard")
                                    .show_ui(ui, |_ui| {})
                            });
                        }
                    });
                } else {
                    ui.radio_value(self, val, val.name());
                }
            }
        })
        .response
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
        let (attr_text, attr_url) = self.tile_type.attr();
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
                let Some(url) = self.tile_type.tile_url(z, x, y) else {
                    continue;
                };
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
        Some(Attribution {
            text: attr_text,
            url: attr_url,
        })
    }
}
