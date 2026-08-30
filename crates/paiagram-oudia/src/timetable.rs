// SPDX-License-Identifier: MPL-2.0
use pest_consume::Parser;

use crate::ir_macros::make_ir_type;
use crate::operation::RootOperationTree;
use crate::time::Time;

/// Also known as `駅扱`.
#[doc(alias = "駅扱")]
#[repr(u32)]
#[derive(Default, Clone, Copy, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ServiceMode {
    /// Also known as `運行なし`.
    ///
    /// The trip does not visit the station
    #[default]
    #[doc(alias = "運行なし")]
    NoOperation = 0,
    /// Also known as `停車`.
    ///
    /// The trip visits the station and stops
    #[doc(alias = "停車")]
    Stop = 1,
    /// Also known as `通過`.
    ///
    /// The trip visits the station but does not stop
    #[doc(alias = "通過")]
    Pass = 2,
}

make_ir_type! {
    /// A timetable entry
    #[derive(Default)]
    struct TimetableEntry as ["Ekijikoku", "駅時刻"];
    pub service_mode as ["駅扱"]: ServiceMode,
    pub arrival_time as ["着時刻"]: Option<Time>,
    pub departure_time as ["発時刻"]: Option<Time>,
    pub track_index as ["着発番線"]: Option<usize>,
    /// Operations associated with this timetable entry.
    operations as ["作業"]: Option<Box<RootOperationTree>>,
}

impl TimetableEntry {
    pub fn operations(&self) -> Option<&RootOperationTree> {
        self.operations.as_deref()
    }
    pub fn operations_mut(&mut self) -> &mut RootOperationTree {
        self.operations.get_or_insert_default()
    }
}

pub mod time {
    use pest_consume::{Error, Parser};

    use super::{ServiceMode, Time, TimetableEntry};

    #[derive(Parser)]
    #[grammar = "timetable.pest"]
    pub struct TimeParser;

    type Result<T> = std::result::Result<T, Error<Rule>>;
    type Node<'i> = pest_consume::Node<'i, Rule, ()>;

    #[pest_consume::parser]
    impl TimeParser {
        fn service_mode(input: Node<'_>) -> Result<ServiceMode> {
            match input.as_str() {
                "0" => Ok(ServiceMode::NoOperation),
                "1" => Ok(ServiceMode::Stop),
                "2" => Ok(ServiceMode::Pass),
                _ => unreachable!(),
            }
        }
        fn arrival_time(input: Node<'_>) -> Result<Time> {
            Time::from_oud_str(input.as_str()).map_err(|e| input.error(e))
        }
        fn departure_time(input: Node<'_>) -> Result<Time> {
            Time::from_oud_str(input.as_str()).map_err(|e| input.error(e))
        }
        fn track_index(input: Node<'_>) -> Result<usize> {
            input.as_str().parse::<usize>().map_err(|e| input.error(e))
        }
        pub fn timetable_entry(input: Node<'_>) -> Result<TimetableEntry> {
            let mut service_mode: ServiceMode = ServiceMode::default();
            let mut arrival_time: Option<Time> = None;
            let mut departure_time: Option<Time> = None;
            let mut track_index: Option<usize> = None;
            for node in input.into_children() {
                match node.as_rule() {
                    Rule::service_mode => service_mode = Self::service_mode(node)?,
                    Rule::arrival_time => arrival_time = Some(Self::arrival_time(node)?),
                    Rule::departure_time => departure_time = Some(Self::departure_time(node)?),
                    Rule::track_index => track_index = Some(Self::track_index(node)?),
                    _ => unreachable!(),
                }
            }
            Ok(TimetableEntry {
                service_mode,
                arrival_time,
                departure_time,
                track_index,
                ..Default::default()
            })
        }
    }
}

pub(crate) fn normalize_times<'a>(mut time_iter: impl Iterator<Item = &'a mut Time> + 'a) {
    let Some(mut previous_time) = time_iter.next().copied() else {
        return;
    };
    for time in time_iter {
        while *time < previous_time {
            *time += Time::from_hms(24, 0, 0);
        }
        previous_time = *time;
    }
}

pub(crate) fn parse_to_timetable_entry(
    input: &'_ str,
) -> Result<TimetableEntry, pest::error::Error<time::Rule>> {
    let a = time::TimeParser::parse(time::Rule::timetable_entry, input)?.single()?;
    Ok(time::TimeParser::timetable_entry(a)?)
}

#[cfg(test)]
mod test {
    use pest_consume::Parser;

    use super::*;
    use crate::ast::{Structure, parse_to_ast};
    use crate::operation::{InsertOperation, parse_to_operation_hierarchy, parse_to_raw_operation};
    type E = Result<(), Box<dyn std::error::Error>>;
    use super::time::{Rule, TimeParser};

    #[test]
    fn parse_times() -> E {
        let s = include_str!("../test/test_times.txt");
        for line in s.lines() {
            let e = TimeParser::parse(Rule::timetable_entry, line)?.single()?;
            let e = TimeParser::timetable_entry(e)?;
            println!("{e:?}");
        }
        Ok(())
    }

    #[test]
    fn comprehend_operations_and_times() -> E {
        let s = include_str!("../test/sample2.oud2");
        let s = Structure::Struct("root".into(), parse_to_ast(s)?);
        let diagrams = s.at(["Rosen", "Dia"]);
        let kudari_trains = diagrams.clone().flat_map(|it| it.at(["Kudari", "Ressya"]));
        let nobori_trains = diagrams.flat_map(|it| it.at(["Nobori", "Ressya"]));
        for train in kudari_trains.chain(nobori_trains) {
            let Structure::Struct(_, vals) = train else {
                panic!()
            };
            let mut times: Vec<_> = train
                .at(["EkiJikoku"])
                .flat_map(|it| {
                    let Structure::Pair(_, vals) = it else {
                        panic!()
                    };
                    vals.iter().map(|it| parse_to_timetable_entry(it).unwrap())
                })
                .collect();
            for (hierarchy, operations) in vals.iter().filter_map(|it| match it {
                Structure::Pair(k, vals) if k.as_ref().starts_with("Operation") => {
                    let hierarchy = parse_to_operation_hierarchy(k.as_ref()).unwrap();
                    let raw_operations =
                        vals.iter().map(|it| parse_to_raw_operation(it.as_ref()).unwrap());
                    Some((hierarchy, raw_operations))
                }
                _ => None,
            }) {
                times.insert_operations(hierarchy, operations);
            }
            for entry in times.iter().filter_map(|it| it.operations()) {
                println!("{entry:#?}")
            }
        }
        Ok(())
    }
}
