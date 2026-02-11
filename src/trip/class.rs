use crate::colors::{DisplayColor, PredefinedColor};
use bevy::{ecs::query::QueryData, prelude::*};
use moonshine_core::prelude::{MapEntities, ReflectMapEntities};

pub struct ClassPlugin;
impl Plugin for ClassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClassResource>();
    }
}

#[derive(Debug, Reflect, Component, Clone, Copy)]
#[reflect(Component)]
pub struct DisplayedStroke {
    pub color: DisplayColor,
    pub width: f32,
}

impl Default for DisplayedStroke {
    fn default() -> Self {
        Self {
            color: DisplayColor::Predefined(PredefinedColor::Emerald),
            width: 1.0,
        }
    }
}

impl DisplayedStroke {
    pub fn egui_stroke(&self, is_dark: bool) -> egui::Stroke {
        egui::Stroke {
            color: self.color.get(is_dark),
            width: self.width,
        }
    }
    pub fn neutral(is_dark: bool) -> egui::Stroke {
        egui::Stroke {
            color: DisplayColor::Predefined(PredefinedColor::Neutral).get(is_dark),
            width: 1.0,
        }
    }
}

#[derive(Default, Reflect, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[relationship_target(relationship = crate::trip::TripClass)]
#[require(Name, DisplayedStroke)]
pub struct Class {
    #[relationship]
    #[entities]
    trips: Vec<Entity>,
}

#[derive(Bundle)]
pub struct ClassBundle {
    pub class: Class,
    pub name: Name,
    pub stroke: DisplayedStroke,
}

#[derive(QueryData)]
pub struct ClassQuery {
    pub vehicles: &'static Class,
    pub name: &'static Name,
    pub stroke: &'static DisplayedStroke,
}

#[derive(Resource)]
pub struct ClassResource {
    // TODO: fix mismatch when reading saves
    pub default_class: Entity,
}

impl FromWorld for ClassResource {
    fn from_world(world: &mut World) -> Self {
        let name = "Default Class";
        let e = world
            .spawn(ClassBundle {
                class: Class::default(),
                name: Name::new(name),
                stroke: DisplayedStroke {
                    color: DisplayColor::Predefined(PredefinedColor::Neutral),
                    width: 1.0,
                },
            })
            .id();
        Self { default_class: e }
    }
}
