// SPDX-License-Identifier: MPL-2.0
//! Intermediate representation of the .oud/oud2 formats.
//! Take a look at [`Root`] to get started.

use std::cmp::{max, min};

use paiagram_oudia_macros::oudia;
use thiserror::Error;

use crate::Structure;
use crate::ir_macros::make_ir_enum;
use crate::operation::{InsertOperation, parse_to_operation_hierarchy, parse_to_raw_operation};
use crate::time::Time;
use crate::timetable::{TimetableEntry, normalize_times};

mod diagram_trips;
mod station;

pub use station::{CrossingCheckRule, OuterTerminal, Station, StationToGraph, Track};

/// The root of the structure
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "")]
pub struct Root {
    /// File type. Usually the software name + version.
    #[oudia(type(single_pair_single_entry = "FileType"))]
    pub file_type: String,
    #[oudia(type(single_pair_single_entry = "FileTypeAppComment"))]
    pub file_type_app_comment: Option<String>,
    /// The route in the file.
    #[oudia(type(single_struct = "Rosen"), alias = "路線")]
    pub route: Route,
    /// Display properties (fonts, colors, widths).
    #[oudia(type(single_struct = "DispProp"), default)]
    pub display_properties: DisplayProperties,
    /// Window layout state saved in the file.
    #[oudia(type(single_struct = "WindowPlacement"), default)]
    pub window_position: WindowPosition,
}

/// Display-related properties. Font and color values are kept as the exact
/// strings the format stores them in.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "DispProp")]
pub struct DisplayProperties {
    /// Horizontal fonts for the timetable screen (one entry per font slot).
    #[oudia(
        type(single_pair_many_entries = "JikokuhyouFont"),
        default = vec![
            "PointTextHeight=9;Facename=Meiryo UI".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI;Bold=1".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI;Itaric=1".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI;Bold=1;Itaric=1".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI".to_string(),
            "PointTextHeight=9;Facename=Meiryo UI".to_string(),
        ]
    )]
    pub timetable_fonts: Vec<String>,
    #[oudia(
        type(single_pair_single_entry = "JikokuhyouVFont"),
        default = "PointTextHeight=9;Facename=@Meiryo UI".to_string()
    )]
    pub timetable_vertical_font: String,
    #[oudia(
        type(single_pair_single_entry = "DiaEkimeiFont"),
        default = "PointTextHeight=9;Facename=Meiryo UI".to_string()
    )]
    pub diagram_station_name_font: String,
    #[oudia(
        type(single_pair_single_entry = "DiaJikokuFont"),
        default = "PointTextHeight=9;Facename=Meiryo UI".to_string()
    )]
    pub diagram_time_font: String,
    #[oudia(
        type(single_pair_single_entry = "DiaRessyaFont"),
        default = "PointTextHeight=9;Facename=Meiryo UI".to_string()
    )]
    pub diagram_trip_font: String,
    #[oudia(
        type(single_pair_single_entry = "OperationTableFont"),
        default = "PointTextHeight=9;Facename=Meiryo UI".to_string()
    )]
    pub operation_table_font: String,
    #[oudia(
        type(single_pair_single_entry = "AllOperationTableJikokuFont"),
        default = "PointTextHeight=8;Facename=Meiryo UI".to_string()
    )]
    pub all_operation_table_time_font: String,
    #[oudia(
        type(single_pair_single_entry = "CommentFont"),
        default = "PointTextHeight=9;Facename=Meiryo UI".to_string()
    )]
    pub comment_font: String,
    #[oudia(
        type(single_pair_single_entry = "DiaMojiColor"),
        default = Color([0, 0, 0, 0])
    )]
    pub diagram_text_color: Color,
    /// Background colors for the diagram screen (one per color slot).
    /// `DiaHaikeiColor` is the pre-`DiaBackColor` name for this field and is
    /// silently ignored when present.
    #[oudia(
        type(single_pair_many_entries = "DiaBackColor"),
        silence_fn = |k: &str| k == "DiaHaikeiColor",
        default = vec![Color([0, 255, 255, 255]); 5]
    )]
    pub diagram_back_colors: Vec<Color>,
    #[oudia(
        type(single_pair_single_entry = "DiaRessyaColor"),
        default = Color([0, 0, 0, 0])
    )]
    pub diagram_train_color: Color,
    #[oudia(
        type(single_pair_single_entry = "DiaJikuColor"),
        default = Color([0, 192, 192, 192])
    )]
    pub diagram_axis_color: Color,
    /// Background colors for the timetable screen (one per color slot).
    #[oudia(
        type(single_pair_many_entries = "JikokuhyouBackColor"),
        default = vec![
            Color([0, 255, 255, 255]),
            Color([0, 240, 240, 240]),
            Color([0, 255, 255, 255]),
            Color([0, 255, 255, 255]),
        ]
    )]
    pub timetable_back_colors: Vec<Color>,
    #[oudia(
        type(single_pair_single_entry = "StdOpeTimeLowerColor"),
        default = Color([0, 224, 224, 255])
    )]
    pub std_ope_time_lower_color: Color,
    #[oudia(
        type(single_pair_single_entry = "StdOpeTimeHigherColor"),
        default = Color([0, 255, 255, 224])
    )]
    pub std_ope_time_higher_color: Color,
    #[oudia(
        type(single_pair_single_entry = "StdOpeTimeUndefColor"),
        default = Color([0, 255, 255, 128])
    )]
    pub std_ope_time_undef_color: Color,
    #[oudia(
        type(single_pair_single_entry = "StdOpeTimeIllegalColor"),
        default = Color([0, 160, 160, 160])
    )]
    pub std_ope_time_illegal_color: Color,
    #[oudia(
        type(single_pair_single_entry = "OperationStringColor"),
        default = Color([0, 0, 0, 0])
    )]
    pub operation_string_color: Color,
    #[oudia(
        type(single_pair_single_entry = "OperationGridColor"),
        default = Color([0, 0, 0, 0])
    )]
    pub operation_grid_color: Color,
    #[oudia(type(single_pair_single_entry = "EkimeiLength"), default = 6)]
    pub station_name_length: i32,
    #[oudia(type(single_pair_single_entry = "JikokuhyouRessyaWidth"), default = 5)]
    pub timetable_train_width: i32,
    #[oudia(type(single_pair_single_entry = "AnySecondIncDec1"), default = 5)]
    pub any_second_step_1: i32,
    #[oudia(type(single_pair_single_entry = "AnySecondIncDec2"), default = 15)]
    pub any_second_step_2: i32,
    #[oudia(type(single_pair_single_entry = "DisplayRessyamei"), default = true)]
    pub display_train_name: bool,
    #[oudia(
        type(single_pair_single_entry = "DisplayOuterTerminalEkimeiOriginSide"),
        default = false
    )]
    pub display_outer_terminal_origin: bool,
    #[oudia(
        type(single_pair_single_entry = "DisplayOuterTerminalEkimeiTerminalSide"),
        default = false
    )]
    pub display_outer_terminal_terminal: bool,
    #[oudia(
        type(single_pair_single_entry = "DiagramDisplayOuterTerminal"),
        default = 0
    )]
    pub diagram_display_outer_terminal: i32,
    #[oudia(type(single_pair_single_entry = "SecondRoundChaku"), default = 0)]
    pub second_round_arrival: i32,
    #[oudia(type(single_pair_single_entry = "SecondRoundHatsu"), default = 0)]
    pub second_round_departure: i32,
    #[oudia(type(single_pair_single_entry = "Display2400"), default = false)]
    pub display_2400: bool,
    #[oudia(type(single_pair_single_entry = "OperationNumberRows"), default = 1)]
    pub operation_number_rows: i32,
    #[oudia(
        type(single_pair_single_entry = "DisplayInOutLinkCode"),
        default = false
    )]
    pub display_in_out_link_code: bool,
}

/// A single saved child window placement.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "ChildWindow")]
pub struct Window {
    #[oudia(type(single_pair_single_entry = "WindowType"))]
    pub window_type: i32,
    #[oudia(type(single_pair_single_entry = "DiaIndex"))]
    pub diagram_index: i32,
    #[oudia(type(single_pair_single_entry = "XPos"))]
    pub x: i32,
    #[oudia(type(single_pair_single_entry = "YPos"))]
    pub y: i32,
    #[oudia(type(single_pair_single_entry = "XSize"))]
    pub width: i32,
    #[oudia(type(single_pair_single_entry = "YSize"))]
    pub height: i32,
}

/// Saved window layout.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "WindowPlacement")]
pub struct WindowPosition {
    #[oudia(type(single_pair_single_entry = "RosenViewWidth"), default = 0)]
    pub route_view_width: i32,
    #[oudia(type(many_structs = "ChildWindow"), default)]
    pub child_windows: Vec<Window>,
}

/// The name of the route
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Rosen", alias = "路線")]
pub struct Route {
    /// The name of the route
    #[oudia(type(single_pair_single_entry = "Rosenmei"), alias = "路線名")]
    pub name: String,
    /// What stations are included in the route
    #[oudia(type(many_structs = "Eki"), alias = "駅")]
    pub stations: Vec<Station>,
    /// The available train classes. E.g., local, express.
    #[oudia(type(many_structs = "Ressyasyubetsu"), alias = "列車種別")]
    pub classes: Vec<Class>,
    /// The diagrams included in this route. Each diagram is a timetable set.
    #[oudia(type(many_structs = "Dia"), alias = "ダイヤ")]
    pub diagrams: Vec<Diagram>,
    /// When to start displaying times on the diagram page.
    #[oudia(type(single_pair_single_entry = "KitenJikoku"), alias = "起点時刻")]
    pub display_start_time: Time,
    #[oudia(type(single_pair_single_entry = "Comment"))]
    pub comment: String,
    #[oudia(
        type(single_pair_single_entry = "KudariDiaAlias"),
        alias = "下りダイヤ別名"
    )]
    pub down_dia_alias: Option<String>,
    #[oudia(
        type(single_pair_single_entry = "NoboriDiaAlias"),
        alias = "上りダイヤ別名"
    )]
    pub up_dia_alias: Option<String>,
    /// Default vertical spacing (in the diagram's Y-coordinate units) between
    /// adjacent stations on the diagram.
    #[oudia(type(single_pair_single_entry = "DiagramDgrYZahyouKyoriDefault"))]
    pub diagram_station_interval_default: i32,
    #[oudia(type(single_pair_single_entry = "EnableOperation"))]
    pub enable_operation: Option<i32>,
    /// Whether operation numbers are shown in reverse order.
    #[oudia(type(single_pair_single_entry = "OperationNumberReverse"))]
    pub operation_number_reverse: Option<bool>,
    /// Whether operations are allowed to cross the diagram's start time.
    #[oudia(type(single_pair_single_entry = "OperationCrossKitenJikoku"))]
    pub operation_crosses_start_time: Option<bool>,
    /// Index of the reference (baseline) diagram.
    #[oudia(type(single_pair_single_entry = "KijunDiaIndex"))]
    pub reference_diagram_index: Option<i32>,
    /// Whether to ignore classes marked as hidden.
    #[oudia(type(single_pair_single_entry = "DisableHiddenSyubetsu"))]
    pub disable_hidden_class: Option<bool>,
}

/// Color. This color is stored in ARGB format.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub fn a(&self) -> u8 {
        self.0[0]
    }
    pub fn r(&self) -> u8 {
        self.0[1]
    }
    pub fn g(&self) -> u8 {
        self.0[2]
    }
    pub fn b(&self) -> u8 {
        self.0[3]
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The OuDia color string is `00BBGGRR`, matching [`Color::from_str`].
        write!(f, "00{:02X}{:02X}{:02X}", self.b(), self.g(), self.r())
    }
}

impl std::str::FromStr for Color {
    type Err = IrConversionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 8 {
            return Err(IrConversionError::ColorConversionError(s.to_string()));
        }
        let (b, g, r) = (
            u8::from_str_radix(&s[2..=3], 16)
                .map_err(|_| IrConversionError::ColorConversionError(s.to_string()))?,
            u8::from_str_radix(&s[4..=5], 16)
                .map_err(|_| IrConversionError::ColorConversionError(s.to_string()))?,
            u8::from_str_radix(&s[6..=7], 16)
                .map_err(|_| IrConversionError::ColorConversionError(s.to_string()))?,
        );
        Ok(Self([0, r, g, b]))
    }
}

/// A train class. E.g., local, express.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Ressyasyubetsu", alias = "列車種別")]
pub struct Class {
    #[oudia(type(single_pair_single_entry = "Syubetsumei"), alias = "種別名")]
    pub name: String,
    /// An optional abbreviation.
    #[oudia(type(single_pair_single_entry = "Ryakusyou"), alias = "略称")]
    pub abbreviation: Option<String>,
    /// The color displayed in diagrams and in the timetable.
    #[oudia(
        type(single_pair_single_entry = "DiagramSenColor"),
        alias = "ダイヤ線Color"
    )]
    pub diagram_line_color: Color,
    #[oudia(
        type(single_pair_single_entry = "JikokuhyouMojiColor"),
        alias = "時刻表文字Color"
    )]
    pub timetable_text_color: Color,
    #[oudia(type(single_pair_single_entry = "JikokuhyouFontIndex"))]
    pub timetable_font_index: i32,
    #[oudia(
        type(single_pair_single_entry = "JikokuhyouBackColor"),
        alias = "時刻表背景Color"
    )]
    pub timetable_background_color: Option<Color>,
    #[oudia(
        type(single_pair_single_entry = "DiagramSenStyle"),
        alias = "ダイヤ線スタイル"
    )]
    pub diagram_line_style: DiagramLineStyle,
    #[oudia(type(single_pair_single_entry = "DiagramSenIsBold"))]
    pub diagram_line_is_bold: Option<bool>,
    #[oudia(
        type(single_pair_single_entry = "StopMarkDrawType"),
        alias = "停車マーク描画タイプ"
    )]
    pub stop_mark_draw_type: StopMarkDrawType,
    #[oudia(type(single_pair_single_entry = "ParentSyubetsuIndex"))]
    pub parent_class_index: Option<i32>,
    #[oudia(type(single_pair_single_entry = "Hidden"))]
    pub hidden: Option<bool>,
}

/// A timetable set.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Dia", alias = "ダイヤ")]
pub struct Diagram {
    #[oudia(type(single_pair_single_entry = "DiaName"))]
    pub name: Option<String>,
    #[oudia(type(twin_struct_multiple_entries(first = "Kudari", second = "Nobori")))]
    pub trips: Vec<Trip>,
    #[oudia(type(single_pair_single_entry = "BackPatternIndex"))]
    pub back_pattern_color_index: Option<i32>,
    #[oudia(type(single_pair_single_entry = "MainBackColorIndex"))]
    pub main_back_color_index: Option<i32>,
    #[oudia(type(single_pair_single_entry = "SubBackColorIndex"))]
    pub sub_back_color_index: Option<i32>,
    #[oudia(type(single_pair_single_entry = "PatternDiagramPreviewEnable"))]
    pub pattern_diagram_preview_enable: Option<bool>,
    #[oudia(type(single_pair_single_entry = "PatternDiagramPreviewCycleSecond"))]
    pub pattern_diagram_preview_cycle_second: Option<i32>,
}

make_ir_enum! {
    enum Direction as ["Houkou", "方向"];
    Up as ["Nobori", "上り"],
    Down as ["Kudari", "下り"],
}

make_ir_enum! {
    enum StationType as ["Ekikibo", "駅規模"];
    Major as ["Ekikibo_Syuyou", "駅規模_主要"],
    Minor as ["Ekikibo_Ippan", "駅規模_一般"],
}

make_ir_enum! {
    /// How a station treats arriving and departing trains in the timetable.
    enum StationTimetableFormat;
    Departure as ["Jikokukeisiki_Hatsu"],
    DepartureAndArrival as ["Jikokukeisiki_Hatsuchaku"],
    DownArrival as ["Jikokukeisiki_KudariChaku"],
    UpArrival as ["Jikokukeisiki_NoboriChaku"],
    DownDepartureAndArrival as ["Jikokukeisiki_KudariHatsuchaku"],
    UpDepartureAndArrival as ["Jikokukeisiki_NoboriHatsuchaku"],
}

make_ir_enum! {
    /// The style used to draw a class's line on the diagram.
    enum DiagramLineStyle;
    Solid as ["SenStyle_Jissen"],
    Dashed as ["SenStyle_Hasen"],
    Dotted as ["SenStyle_Tensen"],
    DashDot as ["SenStyle_Ittensasen"],
}

make_ir_enum! {
    /// When to draw the stop mark on a diagram.
    enum StopMarkDrawType;
    DrawOnStop as ["EStopMarkDrawType_DrawOnStop"],
    Nothing as ["EStopMarkDrawType_Nothing"],
    DrawOnPass as ["EStopMarkDrawType_DrawOnPass"],
}

make_ir_enum! {
    /// When to show train information next to a station on the diagram.
    enum DiagramTrainInfoDisplay;
    Origin as ["DiagramRessyajouhouHyouji_Origin"],
    Anytime as ["DiagramRessyajouhouHyouji_Anytime"],
    Not as ["DiagramRessyajouhouHyouji_Not"],
}

/// A train trip.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Ressya", alias = "列車")]
pub struct Trip {
    #[oudia(type(single_pair_single_entry = "Ressyabangou"), alias = "列車番号")]
    pub name: Option<String>,
    #[oudia(type(single_pair_single_entry = "Bikou"), alias = "備考")]
    pub comment: Option<String>,
    #[oudia(type(single_pair_single_entry = "Ressyamei"), alias = "列車名")]
    pub train_name: Option<String>,
    #[oudia(type(single_pair_single_entry = "Gousuu"), alias = "号数")]
    pub train_number: Option<String>,
    #[oudia(type(single_pair_single_entry = "Canceled"))]
    pub is_canceled: Option<bool>,
    #[oudia(type(single_pair_single_entry = "Houkou"), alias = "方向")]
    pub direction: Direction,
    #[oudia(type(single_pair_single_entry = "Syubetsu"), alias = "種別")]
    pub class_index: usize,
    #[oudia(
        type(single_pair_many_entries = "EkiJikoku"),
        alias       = "駅時刻",
        parse_fn    = parse_timetable_entries,
        silence_fn  = |s: &str| s == "EkiJikoku" || s.starts_with("Operation")
    )]
    pub times: Vec<TimetableEntry>,
}

fn parse_timetable_entries(
    ast: &[Structure<'_>],
) -> Result<Vec<TimetableEntry>, IrConversionError> {
    let times = ast
        .iter()
        .find_map(|node| match node {
            Structure::Pair(k, v) if k == "EkiJikoku" => Some(v),
            _ => None,
        })
        .ok_or(IrConversionError::MissingField {
            processing: std::any::type_name::<TimetableEntry>(),
            missing: "EkiJikoku",
        })?;
    let mut times: Vec<_> = times
        .into_iter()
        .map(|ent| ent.parse::<TimetableEntry>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(IrConversionError::from)?;
    for (key, operations) in ast.iter().filter_map(|node| match node {
        Structure::Pair(k, v) if k.starts_with("Operation") => Some((k, v)),
        _ => None,
    }) {
        let hierarchy = parse_to_operation_hierarchy(key)?;
        let operations =
            operations.iter().map(|s| parse_to_raw_operation(s)).collect::<Result<Vec<_>, _>>()?;
        times.insert_operations(hierarchy, operations);
    }
    normalize_times(times.iter_mut().flat_map(|ent| {
        [ent.arrival_time.as_mut(), ent.departure_time.as_mut()].into_iter().flatten()
    }));
    Ok(times)
}

/// Also known as `運用`.
#[doc(alias = "運用")]
pub struct Rotation<'a> {
    /// Also known as `運用番号`.
    #[doc(alias = "運用番号")]
    pub name: String,
    /// Also known as `列車番号`.
    #[doc(alias = "列車番号")]
    pub trips: Vec<&'a Trip>,
}

fn travelling_duration(curr: &TimetableEntry, next: &TimetableEntry) -> Option<Time> {
    let curr_time = curr.departure_time.or(curr.arrival_time)?;
    let mut next_time = next.arrival_time.or(next.departure_time)?;
    if curr_time > next_time {
        next_time += Time::from_seconds(86400);
    }
    Some(next_time - curr_time)
}

impl Diagram {
    /// 5 minutes
    pub const DEFAULT_INTERVAL_SECONDS: Time = Time::from_seconds(60 * 5);

    /// Return the average travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    pub fn average_interval_durations(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        (0..stations.len().saturating_sub(1)).map(move |idx| {
            let mut avg_seconds: i32 = 0;
            let mut count: i32 = 0;
            for trip in self.trips.iter() {
                let (curr, next) = match trip.direction {
                    Direction::Down => {
                        let Some(next_entry) = trip.times.get(idx + 1) else {
                            continue;
                        };
                        (&trip.times[idx], next_entry)
                    }
                    Direction::Up => {
                        let base = stations.len() - 2 - idx;
                        let Some(next_entry) = trip.times.get(base + 1) else {
                            continue;
                        };
                        (&trip.times[base], next_entry)
                    }
                };
                let Some(diff) = travelling_duration(curr, next) else {
                    continue;
                };
                avg_seconds += diff.seconds();
                count += 1;
            }
            (count != 0).then(|| Time::from_seconds(avg_seconds / count))
        })
    }

    /// Return the extreme travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    fn extrema_interval_durations<const MINIMUM: bool>(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        (0..stations.len().saturating_sub(1)).map(move |idx| {
            let mut extreme = if MINIMUM {
                Time::from_seconds(i32::MAX)
            } else {
                Time::from_seconds(i32::MIN)
            };
            let mut exist: bool = false;
            for trip in self.trips.iter() {
                let (curr, next) = match trip.direction {
                    Direction::Down => {
                        let Some(next_entry) = trip.times.get(idx + 1) else {
                            continue;
                        };
                        (&trip.times[idx], next_entry)
                    }
                    Direction::Up => {
                        let base = stations.len() - 2 - idx;
                        let Some(next_entry) = trip.times.get(base + 1) else {
                            continue;
                        };
                        (&trip.times[base], next_entry)
                    }
                };
                let Some(diff) = travelling_duration(curr, next) else {
                    continue;
                };
                extreme = if MINIMUM {
                    min(extreme, diff)
                } else {
                    max(extreme, diff)
                };
                exist = true;
            }
            exist.then_some(extreme)
        })
    }

    /// Return the minimum travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    pub fn minimum_interval_durations(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        self.extrema_interval_durations::<true>(stations)
    }

    /// Return the maximum travel length between stations
    /// The iterator would yield None the case where no trips traverse an interval.
    pub fn maximum_interval_durations(
        &self,
        stations: &[Station],
    ) -> impl Iterator<Item = Option<Time>> {
        self.extrema_interval_durations::<false>(stations)
    }

    pub fn rotations<'a>(&self, _stations: &[Station]) -> Vec<Rotation<'a>> {
        unimplemented!()
    }
}

#[derive(Debug, Clone, Error)]
pub enum IrConversionError {
    #[error("Missing field '{missing}' when converting AST to '{processing}'")]
    MissingField {
        processing: &'static str,
        missing: &'static str,
    },
    #[error(
        "Index out of bounds when trying to generate '{field}' for '{processing}' (checked index '{index}', but the length is only '{len}')"
    )]
    IndexOutOfBounds {
        field: &'static str,
        processing: &'static str,
        index: usize,
        len: usize,
    },
    #[error("Failed to parse integer: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("Failed to parse timetable entry: {0}")]
    EntryParseError(#[from] pest::error::Error<crate::timetable::time::Rule>),
    #[error("Failed to parse operation: {0}")]
    OperationParseError(#[from] pest::error::Error<crate::operation::operation::Rule>),
    #[error("Failed to parse input to AST: {0}")]
    AstParseError(#[from] pest::error::Error<crate::ast::oudia::Rule>),
    #[error("Unknown token: {0}")]
    UnknownToken(String),
    #[error("Could not convert string {0} to valid color")]
    ColorConversionError(String),
    #[error("Structure is empty while parsing {0}")]
    EmptyError(&'static str),
}

impl From<std::convert::Infallible> for IrConversionError {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::parse_to_ast;
    type E = Result<(), Box<dyn std::error::Error>>;

    pub(crate) fn get_ir() -> Result<Root, IrConversionError> {
        let s = include_str!("../test/sample.oud2");
        let ast = parse_to_ast(s)?;
        <Root as crate::OuDiaIo>::from_structure(ast.as_slice())
    }

    pub(crate) fn get_ir_small() -> Result<Root, IrConversionError> {
        let s = include_str!("../test/sample2.oud2");
        let ast = parse_to_ast(s)?;
        <Root as crate::OuDiaIo>::from_structure(ast.as_slice())
    }

    #[test]
    fn gen_graph() -> E {
        let root = get_ir()?;
        let graph = root.route.stations.to_graph();
        use petgraph::dot::{Config, Dot};
        println!(
            "{:?}",
            Dot::with_attr_getters(
                &graph,
                &[Config::EdgeNoLabel, Config::NodeNoLabel],
                &|_, _| String::new(),
                &|_, (_, stn)| format!("label = \"{}\"", stn.name)
            )
        );
        Ok(())
    }

    #[test]
    fn test_parse_ast_to_ir() -> E {
        let ir = get_ir()?;
        println!("{ir:#?}");
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_rotations() -> E {
        let ir = get_ir()?;
        if let Some(diagram) = ir.route.diagrams.first() {
            let mut rotations = diagram.rotations(&ir.route.stations);
            rotations.sort_by_key(|it| it.name.clone());
            for Rotation { name, trips } in rotations.into_iter() {
                println!("========== Rotation '{name}' ==========");
                for trip in trips {
                    println!("{}", trip.name.as_deref().unwrap_or("<unnamed>"))
                }
            }
        }
        Ok(())
    }

    #[test]
    fn average_interval_durations() -> E {
        let ir = get_ir()?;
        let diagram = &ir.route.diagrams[0];
        for time in diagram.average_interval_durations(&ir.route.stations) {
            println!("{:#?}", time);
        }
        Ok(())
    }

    #[test]
    fn minimum_interval_durations() -> E {
        let ir = get_ir()?;
        let diagram = &ir.route.diagrams[0];
        for time in diagram.minimum_interval_durations(&ir.route.stations) {
            println!("{:#?}", time);
        }
        Ok(())
    }

    #[test]
    fn maximum_interval_durations() -> E {
        let ir = get_ir()?;
        let diagram = &ir.route.diagrams[0];
        for time in diagram.maximum_interval_durations(&ir.route.stations) {
            println!("{:#?}", time);
        }
        Ok(())
    }
}
