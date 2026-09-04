// SPDX-License-Identifier: MPL-2.0
//! Stations, their tracks, through-service terminals, and crossing rules.

use paiagram_oudia_macros::oudia;

use super::{DiagramTrainInfoDisplay, StationTimetableFormat, StationType};

/// A station on the route.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "Eki", alias = "駅")]
pub struct Station {
    #[oudia(type(single_pair_single_entry = "Ekimei"), alias = "駅名")]
    pub name: String,
    /// The abbreviation used in timetables.
    #[oudia(
        type(single_pair_single_entry = "EkimeiJikokuRyaku"),
        alias = "駅名時刻略"
    )]
    pub timetable_abbreviation: Option<String>,
    /// The abbreviation used in diagrams.
    #[oudia(
        type(single_pair_single_entry = "EkimeiDiaRyaku"),
        alias = "駅名ダイヤ略"
    )]
    pub diagram_abbreviation: Option<String>,
    /// Stations that branch off at certain points may repeat themselves on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station. Please also note that
    /// the name `BrunchCoreEkiIndex` contains a spelling mistake. It should be
    /// `branch` instead of `brunch`.
    #[oudia(type(single_pair_single_entry = "BrunchCoreEkiIndex"))]
    pub branch_index: Option<usize>,
    /// Whether this branch repeats on the opposite side of the diagram.
    #[oudia(type(single_pair_single_entry = "BrunchOpposite"))]
    pub branch_opposite: Option<bool>,
    /// Diagrams representing loop lines may repeat certain stations on
    /// the diagram. This index refers to the other station in the station list
    /// that should be treated as if it is this station.
    #[oudia(type(single_pair_single_entry = "LoopOriginEkiIndex"))]
    pub loop_index: Option<usize>,
    /// The tracks of the station
    #[oudia(type(single_struct_many_entries = "EkiTrack2Cont"))]
    pub tracks: Vec<Track>,
    #[oudia(type(single_pair_single_entry = "Ekikibo"), alias = "駅規模")]
    pub station_type: StationType,
    #[oudia(
        type(single_pair_single_entry = "Ekijikokukeisiki"),
        alias = "駅時刻形式"
    )]
    pub station_timetable_format: StationTimetableFormat,
    /// The main track for down (Kudari) trains, as a track index.
    #[oudia(type(single_pair_single_entry = "DownMain"))]
    pub down_main_track_index: Option<i32>,
    /// The main track for up (Nobori) trains, as a track index.
    #[oudia(type(single_pair_single_entry = "UpMain"))]
    pub up_main_track_index: Option<i32>,
    #[oudia(type(single_pair_single_entry = "DiagramTrackDisplay"))]
    pub diagram_track_display: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouTrackDisplayKudari"))]
    pub timetable_track_display_down: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouTrackDisplayNobori"))]
    pub timetable_track_display_up: Option<bool>,
    /// Marks a boundary between two parts of a line (e.g. where a loop joins).
    #[oudia(type(single_pair_single_entry = "Kyoukaisen"))]
    pub is_boundary: Option<bool>,
    /// Whether the loop repeats this station on the opposite side.
    #[oudia(type(single_pair_single_entry = "LoopOpposite"))]
    pub loop_opposite: Option<bool>,
    #[oudia(
        type(single_pair_single_entry = "DiagramRessyajouhouHyoujiKudari"),
        alias = "ダイヤ列車情報表示下り"
    )]
    pub diagram_train_info_display_down: Option<DiagramTrainInfoDisplay>,
    #[oudia(
        type(single_pair_single_entry = "DiagramRessyajouhouHyoujiNobori"),
        alias = "ダイヤ列車情報表示上り"
    )]
    pub diagram_train_info_display_up: Option<DiagramTrainInfoDisplay>,
    /// Index into the diagram background color list for the next station.
    #[oudia(type(single_pair_single_entry = "DiagramColorNextEki"))]
    pub next_station_color_index: Option<i32>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouTrackOmit"))]
    pub timetable_track_omit: Option<bool>,
    /// Whether to show times at this station in the "box diagram" operation table.
    #[oudia(type(single_pair_single_entry = "OperationTableDisplayJikoku"))]
    pub operation_table_display_time: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouOperationOrigin"))]
    pub timetable_operation_origin: Option<i32>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouOperationTerminal"))]
    pub timetable_operation_terminal: Option<i32>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouOperationOriginDownBeforeUpAfter"))]
    pub timetable_operation_origin_down_before_up_after: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouOperationOriginDownAfterUpBefore"))]
    pub timetable_operation_origin_down_after_up_before: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouOperationTerminalDownBeforeUpAfter"))]
    pub timetable_operation_terminal_down_before_up_after: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouOperationTerminalDownAfterUpBefore"))]
    pub timetable_operation_terminal_down_after_up_before: Option<bool>,
    /// Per-track flags hiding a track from the diagram track listing.
    #[oudia(type(single_pair_many_entries = "DiagramTrackOmit"))]
    pub diagram_track_omit: Vec<bool>,
    /// Down-direction per-station arrival/departure time display flags.
    #[oudia(type(single_pair_many_entries = "JikokuhyouJikokuDisplayKudari"))]
    pub timetable_time_display_down: Vec<i32>,
    /// Up-direction per-station arrival/departure time display flags.
    #[oudia(type(single_pair_many_entries = "JikokuhyouJikokuDisplayNobori"))]
    pub timetable_time_display_up: Vec<i32>,
    /// Down-direction per-station class-change display settings.
    #[oudia(type(single_pair_many_entries = "JikokuhyouSyubetsuChangeDisplayKudari"))]
    pub timetable_class_change_display_down: Vec<i32>,
    /// Up-direction per-station class-change display settings.
    #[oudia(type(single_pair_many_entries = "JikokuhyouSyubetsuChangeDisplayNobori"))]
    pub timetable_class_change_display_up: Vec<i32>,
    /// Down-direction per-station through-service display settings.
    #[oudia(type(single_pair_many_entries = "JikokuhyouOuterDisplayKudari"))]
    pub timetable_outer_display_down: Vec<i32>,
    /// Up-direction per-station through-service display settings.
    #[oudia(type(single_pair_many_entries = "JikokuhyouOuterDisplayNobori"))]
    pub timetable_outer_display_up: Vec<i32>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouNyuusenJikokuDisplayKudari"))]
    pub timetable_incoming_display_down: Option<bool>,
    #[oudia(type(single_pair_single_entry = "JikokuhyouNyuusenJikokuDisplayNobori"))]
    pub timetable_incoming_display_up: Option<bool>,
    #[oudia(type(many_structs = "OuterTerminal"))]
    pub outer_terminals: Vec<OuterTerminal>,
    #[oudia(type(many_structs = "CrossingCheckRule"))]
    pub crossing_check_rules: Vec<CrossingCheckRule>,
}

pub trait StationToGraph {
    fn merge_duplicate(&self) -> Vec<&Station>;
    fn to_graph<'a>(&'a self) -> petgraph::graph::UnGraph<&'a Station, ()>;
}

impl StationToGraph for [Station] {
    fn merge_duplicate(&self) -> Vec<&Station> {
        let mut ret: Vec<&Station> = self.iter().collect();
        for curr in 0..ret.len() {
            let Some(ext) = ret[curr].branch_index.or(ret[curr].loop_index) else {
                continue;
            };
            if let Some(stn) = ret.get(ext).copied() {
                ret[curr] = stn;
            }
        }
        ret
    }
    fn to_graph<'a>(&'a self) -> petgraph::graph::UnGraph<&'a Station, ()> {
        // only merge stations based on branch index and loop index
        let mut graph = petgraph::graph::UnGraph::new_undirected();
        let mut idxs: Vec<_> = self.iter().map(|stn| graph.add_node(stn)).collect();
        for curr in 0..idxs.len() {
            let Some(ext) = self[curr].branch_index.or(self[curr].loop_index) else {
                continue;
            };
            if let Some(node_idx) = idxs.get(ext).copied() {
                let old_idx = idxs[curr];
                idxs[curr] = node_idx;
                graph.remove_node(old_idx);
            }
        }
        for [prev, next] in idxs.array_windows::<2>().copied() {
            if graph.node_weight(prev).is_some() && graph.node_weight(next).is_some() {
                graph.update_edge(prev, next, ());
            }
        }
        graph
    }
}

/// A station track.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "EkiTrack2")]
pub struct Track {
    #[oudia(type(single_pair_single_entry = "TrackName"))]
    pub name: String,
    #[oudia(type(single_pair_single_entry = "TrackRyakusyou"), alias = "Track略称")]
    pub abbreviation: String,
    /// The abbreviation used for up (Nobori) trains on this track.
    #[oudia(
        type(single_pair_single_entry = "TrackNoboriRyakusyou"),
        alias = "Track上り略称"
    )]
    pub up_abbreviation: Option<String>,
}

/// An outer terminal a station connects to for through services.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "OuterTerminal", alias = "路線外終端")]
pub struct OuterTerminal {
    #[oudia(type(single_pair_single_entry = "OuterTerminalEkimei"))]
    pub name: String,
    #[oudia(type(single_pair_single_entry = "OuterTerminalJikokuRyaku"))]
    pub timetable_abbreviation: String,
    #[oudia(type(single_pair_single_entry = "OuterTerminalDiaRyaku"))]
    pub diagram_abbreviation: String,
}

/// A rule for checking train crossings on the diagram.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
#[oudia(key = "CrossingCheckRule")]
pub struct CrossingCheckRule {
    #[oudia(type(single_pair_single_entry = "Caption"))]
    pub caption: String,
    #[oudia(type(single_pair_single_entry = "Enable"))]
    pub enable: bool,
    #[oudia(type(single_pair_single_entry = "HeadwaySecond"))]
    pub headway_second: i32,
    #[oudia(type(single_pair_single_entry = "HeadwaySecondMinimum"))]
    pub headway_second_minimum: i32,
    #[oudia(type(single_pair_single_entry = "BeforeFromTrackContentCont"))]
    pub before_from_track_contents: String,
    #[oudia(type(single_pair_single_entry = "BeforeToTrackContentCont"))]
    pub before_to_track_contents: String,
    #[oudia(type(single_pair_single_entry = "BeforeIsArrival"))]
    pub before_is_arrival: bool,
    #[oudia(type(single_pair_single_entry = "BeforeIsTsuuka"))]
    pub before_is_pass: bool,
    #[oudia(type(single_pair_single_entry = "AfterFromTrackContentCont"))]
    pub after_from_track_contents: String,
    #[oudia(type(single_pair_single_entry = "AfterToTrackContentCont"))]
    pub after_to_track_contents: String,
    #[oudia(type(single_pair_single_entry = "AfterIsArrival"))]
    pub after_is_arrival: bool,
    #[oudia(type(single_pair_single_entry = "AfterIsTsuuka"))]
    pub after_is_pass: bool,
}
