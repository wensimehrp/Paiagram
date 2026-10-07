use std::collections::HashMap;

use egui::*;
use rquickjs::{Ctx, Function, IntoJs, Object, Value};
use serde::Deserialize;

pub type ReturnVariableName = String;
pub type ReturnValueMap = HashMap<ReturnVariableName, ConfigUiReturnValue>;

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConfigUi {
    Text {
        desc: String,
    },
    Separator,
    CollapsingHeader {
        variable: ReturnVariableName,
        desc: String,
        items: Vec<ConfigUi>,
    },
    Radio {
        variable: ReturnVariableName,
        desc: String,
        items: Vec<String>,
        default: usize,
    },
    Tickbox {
        variable: ReturnVariableName,
        desc: String,
        default: bool,
    },
    TextInput {
        variable: ReturnVariableName,
        desc: String,
        placeholder: Option<String>,
        default: Option<String>,
    },
    DragValue {
        variable: ReturnVariableName,
        desc: String,
        integer: bool,
        min: f64,
        max: f64,
        default: f64,
    },
}

impl ConfigUi {
    /// Decodes a UI definition from a JavaScript value (an array of element objects) by
    /// round-tripping it through `JSON.stringify` and `serde_json`. `undefined` yields no elements.
    pub fn parse_many<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Vec<ConfigUi>> {
        let json_object: Object = ctx.globals().get("JSON")?;
        let stringify: Function = json_object.get("stringify")?;
        let json: Option<String> = stringify.call((value,))?;
        let Some(json) = json else {
            return Ok(Vec::new());
        };
        serde_json::from_str(&json).map_err(|err| {
            rquickjs::Error::new_from_js_message("JSON", "ConfigUi", err.to_string())
        })
    }

    fn desc(&self) -> Option<&String> {
        match self {
            Self::Text { desc } => Some(desc),
            Self::Separator => None,
            // special case for collapsing header as we want to display it on the header
            Self::CollapsingHeader { .. } => None,
            Self::Radio { desc, .. } => Some(desc),
            Self::Tickbox { .. } => None,
            Self::TextInput { desc, .. } => Some(desc),
            Self::DragValue { desc, .. } => Some(desc),
        }
    }

    fn get_return_value(&self) -> Option<(ReturnVariableName, ConfigUiReturnValue)> {
        match self {
            Self::Radio {
                variable, default, ..
            } => Some((
                variable.clone(),
                ConfigUiReturnValue::Number(*default as f64),
            )),
            Self::Tickbox {
                variable, default, ..
            } => Some((variable.clone(), ConfigUiReturnValue::Boolean(*default))),
            Self::TextInput {
                variable, default, ..
            } => Some((
                variable.clone(),
                ConfigUiReturnValue::String(default.as_ref().map_or_default(String::clone)),
            )),
            Self::DragValue {
                variable, default, ..
            } => Some((variable.clone(), ConfigUiReturnValue::Number(*default))),
            Self::CollapsingHeader {
                variable, items, ..
            } => Some((
                variable.clone(),
                ConfigUiReturnValue::Map(
                    items.iter().filter_map(ConfigUi::get_return_value).collect(),
                ),
            )),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConfigUiReturnValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Map(ReturnValueMap),
}

impl<'js> IntoJs<'js> for ConfigUiReturnValue {
    /// Unwraps the enum so scripts read `paiagram.return_values.foo` directly (a number, string,
    /// boolean, or nested object) instead of `paiagram.return_values.foo.number`.
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        match self {
            Self::String(value) => value.into_js(ctx),
            Self::Number(value) => value.into_js(ctx),
            Self::Boolean(value) => value.into_js(ctx),
            Self::Map(map) => map.into_js(ctx),
        }
    }
}

/// Builds the initial return values (the widgets' defaults) for a UI definition.
pub fn default_return_values(ui_definition: &[ConfigUi]) -> ReturnValueMap {
    ui_definition.iter().filter_map(ConfigUi::get_return_value).collect()
}

pub struct ConfigUiList<'a> {
    pub ui_definition: &'a [ConfigUi],
    pub return_value: &'a mut ReturnValueMap,
}

impl<'a> ConfigUiList<'a> {
    pub fn show_ui(mut self, ui: &mut Ui) {
        for elem in self.ui_definition {
            if let Some(desc) = elem.desc() {
                ui.label(desc);
                Frame::NONE
                    .inner_margin(Margin {
                        left: 12,
                        ..Default::default()
                    })
                    .show(ui, |ui| self.show_inner(ui, elem));
            } else {
                self.show_inner(ui, elem);
            }
        }
    }
    fn show_inner(&mut self, ui: &mut Ui, elem: &ConfigUi) {
        match elem {
            ConfigUi::Text { .. } => {
                // already handled
            }
            ConfigUi::Separator => {
                ui.separator();
            }
            ConfigUi::CollapsingHeader {
                variable,
                desc,
                items,
                ..
            } => {
                let Some(ConfigUiReturnValue::Map(map)) = self.return_value.get_mut(variable)
                else {
                    unreachable!();
                };
                ui.collapsing(desc, |ui| {
                    ConfigUiList {
                        ui_definition: items,
                        return_value: map,
                    }
                    .show_ui(ui);
                });
            }
            ConfigUi::Radio {
                variable, items, ..
            } => {
                let Some(ConfigUiReturnValue::Number(current_value)) =
                    self.return_value.get_mut(variable)
                else {
                    unreachable!()
                };
                ui.horizontal_wrapped(|ui| {
                    for (idx, item) in items.iter().enumerate() {
                        ui.radio_value(current_value, idx as f64, item);
                    }
                });
            }
            ConfigUi::Tickbox { variable, desc, .. } => {
                let Some(ConfigUiReturnValue::Boolean(current_value)) =
                    self.return_value.get_mut(variable)
                else {
                    unreachable!()
                };
                ui.checkbox(current_value, desc);
            }
            ConfigUi::TextInput {
                variable,
                placeholder,
                ..
            } => {
                let Some(ConfigUiReturnValue::String(current_value)) =
                    self.return_value.get_mut(variable)
                else {
                    unreachable!()
                };
                TextEdit::singleline(current_value)
                    .hint_text(placeholder.as_ref().map_or_default(String::as_str))
                    .show(ui);
            }
            ConfigUi::DragValue {
                variable,
                integer,
                min,
                max,
                ..
            } => {
                let Some(ConfigUiReturnValue::Number(current_value)) =
                    self.return_value.get_mut(variable)
                else {
                    unreachable!()
                };
                let mut widget =
                    DragValue::new(current_value).range(*min..=*max).clamp_existing_to_range(true);
                if *integer {
                    widget = widget.min_decimals(0).max_decimals(0)
                }
                ui.add(widget);
            }
        }
    }
}
