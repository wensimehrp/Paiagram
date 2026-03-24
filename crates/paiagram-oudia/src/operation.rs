use pest_consume::Parser;
use smallvec::SmallVec;
use thiserror::Error;

use crate::{ast::SerializeToOud, time::Time, timetable::TimetableEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BeforeAfter {
    B(usize),
    A(usize),
}

impl BeforeAfter {
    fn index(self) -> usize {
        match self {
            Self::A(i) | Self::B(i) => i,
        }
    }
}

pub trait InsertOperation<'a> {
    fn insert_operations<'b>(
        &mut self,
        hierarchy: impl IntoIterator<Item = BeforeAfter>,
        operations: impl IntoIterator<Item = RawOperation<'b>>,
    ) where
        'b: 'a;
}

macro_rules! impl_get_before_after {
    ($x:ident, $native:ident, $native_type:ty) => {
        #[derive(Debug, Clone, PartialEq, Default, Eq)]
        pub struct $x {
            pub ops: SmallVec<[$native_type; 1]>,
            pub befores: Vec<BeforeOperationTree>,
            pub afters: Vec<AfterOperationTree>,
        }

        impl<'a> InsertOperation<'a> for $x {
            fn insert_operations<'b>(
                &mut self,
                hierarchy: impl IntoIterator<Item = BeforeAfter>,
                operations: impl IntoIterator<Item = RawOperation<'b>>,
            ) where
                'b: 'a,
            {
                let mut hierarchy = hierarchy.into_iter();
                let Some(index) = hierarchy.next() else {
                    // we are at the end of the journey!
                    for operation in operations {
                        self.ops.push(
                            operation
                                .try_into()
                                .expect("failed to parse operation at leaf node"),
                        );
                    }
                    return;
                };
                // the journey goes on...
                match index {
                    BeforeAfter::B(i) => {
                        if i >= self.befores.len() {
                            self.befores
                                .resize_with(i + 1, BeforeOperationTree::default);
                        }
                        self.befores[i].insert_operations(hierarchy, operations)
                    }
                    BeforeAfter::A(i) => {
                        if i >= self.afters.len() {
                            self.afters.resize_with(i + 1, AfterOperationTree::default);
                        }
                        self.afters[i].insert_operations(hierarchy, operations)
                    }
                }
            }
        }
    };
}

impl_get_before_after!(BeforeOperationTree, befores, BeforeOperation);
impl_get_before_after!(AfterOperationTree, afters, AfterOperation);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RootOperationTree {
    pub befores: BeforeOperationTree,
    pub afters: AfterOperationTree,
}

impl<'e> InsertOperation<'e> for Vec<TimetableEntry> {
    /// Insert the operations for the timetable.
    /// Note that this method would panic if the indexes don't match.
    fn insert_operations<'a>(
        &mut self,
        hierarchy: impl IntoIterator<Item = BeforeAfter>,
        operations: impl IntoIterator<Item = RawOperation<'a>>,
    ) where
        'a: 'e,
    {
        let mut hierarchy = hierarchy.into_iter();
        let root_index = hierarchy.next().unwrap();
        let entry = &mut self[root_index.index()];
        let root_tree = entry.operations_mut();
        match root_index {
            BeforeAfter::B(_) => {
                root_tree.befores.insert_operations(hierarchy, operations);
            }
            BeforeAfter::A(_) => {
                root_tree.afters.insert_operations(hierarchy, operations);
            }
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Type0 = 0,
    Type1,
    Type2,
    Type3,
    Type4,
    Type5,
    Type6 = 6,
}

impl std::str::FromStr for OperationType {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::Type0),
            "1" => Ok(Self::Type1),
            "2" => Ok(Self::Type2),
            "3" => Ok(Self::Type3),
            "4" => Ok(Self::Type4),
            "5" => Ok(Self::Type5),
            "6" => Ok(Self::Type6),
            _ => Err("Operation type should be a number between 0~6"),
        }
    }
}

#[derive(Debug, Error)]
pub enum OperationParseError {
    #[error("missing required parameter for {field}")]
    MissingRequiredParam { field: &'static str },
    #[error("invalid {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("invalid optional parameter at index {idx}: {message}")]
    InvalidOptionalParam { idx: usize, message: String },
    #[error("invalid OUD time '{value}': {source}")]
    InvalidOudTime {
        value: String,
        source: std::num::ParseIntError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawOperation<'a> {
    pub operation_type: OperationType,
    pub params: [Option<&'a str>; 5],
}

impl<'a> SerializeToOud for RawOperation<'a> {
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()> {
        buf.write_all((self.operation_type as u32).to_string().as_bytes())?;
        buf.write_all(b"/")?;
        buf.write_all(self.params[0].unwrap_or("").as_bytes())?;
        buf.write_all(b"$")?;
        buf.write_all(self.params[0].unwrap_or("").as_bytes())?;
        buf.write_all(b"/")?;
        buf.write_all(self.params[0].unwrap_or("").as_bytes())?;
        buf.write_all(b"$")?;
        buf.write_all(self.params[0].unwrap_or("").as_bytes())?;
        buf.write_all(b"/")?;
        buf.write_all(self.params[0].unwrap_or("").as_bytes())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShuntOperation {
    pub track_index: usize,
    pub departure_time: Option<Time>,
    pub arrival_time: Option<Time>,
    pub display_time: bool,
}

impl TryFrom<&[Option<&str>]> for ShuntOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&str>]) -> Result<Self, Self::Error> {
        let track_index = parse_required::<usize>(value, 0, "track index")?;
        let departure_time = parse_time_opt(value.get(1).copied().flatten())?;
        let arrival_time = parse_time_opt(value.get(2).copied().flatten())?;
        let display_time = parse_bool_required(value, 3, "display time flag")?;
        Ok(Self {
            track_index,
            departure_time,
            arrival_time,
            display_time,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoupleOperation {
    pub add_to_front: bool,
    pub time: Option<Time>,
}

impl TryFrom<&[Option<&str>]> for CoupleOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            add_to_front: parse_bool_with_default(value.get(0).copied().flatten(), false)?,
            time: parse_time_opt(value.get(1).copied().flatten())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoupleOperation {
    pub position_index: usize,
    pub count: usize,
    pub time: Option<Time>,
}

impl TryFrom<&[Option<&str>]> for DecoupleOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            position_index: parse_required::<usize>(value, 0, "release position")?,
            count: parse_required::<usize>(value, 1, "release index count")?,
            time: parse_time_opt(value.get(2).copied().flatten())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterFromDepotOperation {
    pub time: Option<Time>,
    pub link_code: Option<String>,
    pub operation_numbers: SmallVec<[String; 2]>,
}

impl<'a> TryFrom<&[Option<&'a str>]> for EnterFromDepotOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&'a str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            time: parse_time_opt(value.get(0).copied().flatten())?,
            link_code: parse_link_code(value.get(1).copied().flatten()),
            operation_numbers: parse_operation_numbers(value.get(2).copied().flatten()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitToDepotOperation {
    pub time: Option<Time>,
    pub link_code: Option<String>,
}

impl<'a> TryFrom<&[Option<&'a str>]> for ExitToDepotOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&'a str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            time: parse_time_opt(value.get(0).copied().flatten())?,
            link_code: parse_link_code(value.get(1).copied().flatten()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeforeEnterFromExternalRouteOperation {
    pub station_index: usize,
    pub time: Option<Time>,
    pub arrival_time: Option<Time>,
    pub link_code: Option<String>,
    pub operation_numbers: SmallVec<[String; 2]>,
}

impl<'a> TryFrom<&[Option<&'a str>]> for BeforeEnterFromExternalRouteOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&'a str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            station_index: parse_required::<usize>(value, 0, "outer terminal index")?,
            time: parse_time_opt(value.get(1).copied().flatten())?,
            arrival_time: parse_time_opt(value.get(2).copied().flatten())?,
            link_code: parse_link_code(value.get(3).copied().flatten()),
            operation_numbers: parse_operation_numbers(value.get(4).copied().flatten()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitToExternalRouteOperation {
    pub station_index: usize,
    pub time: Option<Time>,
    pub arrival_time: Option<Time>,
    pub link_code: Option<String>,
}

impl<'a> TryFrom<&[Option<&'a str>]> for ExitToExternalRouteOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&'a str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            station_index: parse_required::<usize>(value, 0, "outer terminal index")?,
            time: parse_time_opt(value.get(1).copied().flatten())?,
            arrival_time: parse_time_opt(value.get(2).copied().flatten())?,
            link_code: parse_link_code(value.get(3).copied().flatten()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuePreviousTripOperation {
    pub time: Option<Time>,
    pub operation_numbers: SmallVec<[String; 2]>,
    pub next_junction_type: Option<i32>,
}

impl<'a> TryFrom<&[Option<&'a str>]> for ContinuePreviousTripOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&'a str>]) -> Result<Self, Self::Error> {
        Ok(Self {
            time: parse_time_opt(value.get(0).copied().flatten())?,
            operation_numbers: parse_operation_numbers(value.get(1).copied().flatten()),
            next_junction_type: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeOperationNumberOperation {
    pub operation_numbers: SmallVec<[String; 2]>,
    pub reverse: bool,
}

impl<'a> TryFrom<&[Option<&'a str>]> for ChangeOperationNumberOperation {
    type Error = OperationParseError;
    fn try_from(value: &[Option<&'a str>]) -> Result<Self, Self::Error> {
        let operation_numbers = parse_operation_numbers(value.get(0).copied().flatten());
        let reverse = parse_bool_opt(value.get(1).copied().flatten())
            .unwrap_or_else(|| operation_numbers.is_empty());
        Ok(Self {
            operation_numbers,
            reverse,
        })
    }
}

#[repr(u32)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeforeOperation {
    Shunt(ShuntOperation) = 0,
    Couple(CoupleOperation) = 1,
    Decouple(DecoupleOperation) = 2,
    EnterFromDepot(EnterFromDepotOperation) = 3,
    EnterFromExternalRoute(BeforeEnterFromExternalRouteOperation) = 4,
    ContinuePreviousTrip(ContinuePreviousTripOperation) = 5,
    ChangeOperationNumber(ChangeOperationNumberOperation) = 6,
}

impl<'a> TryFrom<RawOperation<'a>> for BeforeOperation {
    type Error = OperationParseError;
    fn try_from(value: RawOperation<'a>) -> Result<Self, Self::Error> {
        Ok(match value.operation_type {
            OperationType::Type0 => Self::Shunt(value.params.as_slice().try_into()?),
            OperationType::Type1 => Self::Couple(value.params.as_slice().try_into()?),
            OperationType::Type2 => Self::Decouple(value.params.as_slice().try_into()?),
            OperationType::Type3 => Self::EnterFromDepot(value.params.as_slice().try_into()?),
            OperationType::Type4 => {
                Self::EnterFromExternalRoute(value.params.as_slice().try_into()?)
            }
            OperationType::Type5 => Self::ContinuePreviousTrip(value.params.as_slice().try_into()?),
            OperationType::Type6 => {
                Self::ChangeOperationNumber(value.params.as_slice().try_into()?)
            }
        })
    }
}

#[repr(u32)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterOperation {
    Shunt(ShuntOperation) = 0,
    Couple(CoupleOperation) = 1,
    Decouple(DecoupleOperation) = 2,
    ExitToDepot(ExitToDepotOperation) = 3,
    ExitToExternalRoute(ExitToExternalRouteOperation) = 4,
    ContinuePreviousTrip(ContinuePreviousTripOperation) = 5,
    ChangeOperationNumber(ChangeOperationNumberOperation) = 6,
}

impl<'a> TryFrom<RawOperation<'a>> for AfterOperation {
    type Error = OperationParseError;
    fn try_from(value: RawOperation<'a>) -> Result<Self, Self::Error> {
        match value.operation_type {
            OperationType::Type0 => Ok(Self::Shunt(value.params.as_slice().try_into()?)),
            OperationType::Type1 => Ok(Self::Couple(value.params.as_slice().try_into()?)),
            OperationType::Type2 => Ok(Self::Decouple(value.params.as_slice().try_into()?)),
            OperationType::Type3 => Ok(Self::ExitToDepot(value.params.as_slice().try_into()?)),
            OperationType::Type4 => Ok(Self::ExitToExternalRoute(
                value.params.as_slice().try_into()?,
            )),
            OperationType::Type5 => Ok(Self::ContinuePreviousTrip(ContinuePreviousTripOperation {
                time: parse_time_opt(value.params.get(0).copied().flatten())?,
                operation_numbers: SmallVec::new(),
                next_junction_type: parse_optional::<i32>(value.params.as_slice(), 1)?,
            })),
            OperationType::Type6 => Ok(Self::ChangeOperationNumber(
                value.params.as_slice().try_into()?,
            )),
        }
    }
}

fn parse_non_empty(s: Option<&str>) -> Option<&str> {
    s.and_then(|it| {
        let trimmed = it.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn parse_required<T: std::str::FromStr>(
    value: &[Option<&str>],
    idx: usize,
    field_name: &'static str,
) -> Result<T, OperationParseError>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    let s = parse_non_empty(value.get(idx).copied().flatten())
        .ok_or(OperationParseError::MissingRequiredParam { field: field_name })?;
    s.parse::<T>()
        .map_err(|e| OperationParseError::InvalidField {
            field: field_name,
            message: e.to_string(),
        })
}

fn parse_optional<T: std::str::FromStr>(
    value: &[Option<&str>],
    idx: usize,
) -> Result<Option<T>, OperationParseError>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    let Some(s) = parse_non_empty(value.get(idx).copied().flatten()) else {
        return Ok(None);
    };
    let parsed = s
        .parse::<T>()
        .map_err(|e| OperationParseError::InvalidOptionalParam {
            idx,
            message: e.to_string(),
        })?;
    Ok(Some(parsed))
}

fn parse_time_opt(s: Option<&str>) -> Result<Option<Time>, OperationParseError> {
    let Some(s) = parse_non_empty(s) else {
        return Ok(None);
    };
    Time::from_oud_str(s)
        .map(Some)
        .map_err(|source| OperationParseError::InvalidOudTime {
            value: s.to_string(),
            source,
        })
}

fn parse_bool_raw(s: &str) -> Result<bool, OperationParseError> {
    match s {
        "1" | "true" | "True" | "TRUE" => Ok(true),
        "0" | "false" | "False" | "FALSE" => Ok(false),
        _ => Err(OperationParseError::InvalidField {
            field: "bool",
            message: format!("invalid bool value '{s}'"),
        }),
    }
}

fn parse_bool_opt(s: Option<&str>) -> Option<bool> {
    parse_non_empty(s).and_then(|it| parse_bool_raw(it).ok())
}

fn parse_bool_with_default(s: Option<&str>, default: bool) -> Result<bool, OperationParseError> {
    let Some(s) = parse_non_empty(s) else {
        return Ok(default);
    };
    parse_bool_raw(s)
}

fn parse_bool_required(
    value: &[Option<&str>],
    idx: usize,
    field_name: &'static str,
) -> Result<bool, OperationParseError> {
    let s = parse_non_empty(value.get(idx).copied().flatten())
        .ok_or(OperationParseError::MissingRequiredParam { field: field_name })?;
    parse_bool_raw(s).map_err(|e| OperationParseError::InvalidField {
        field: field_name,
        message: e.to_string(),
    })
}

fn parse_link_code<'a>(s: Option<&'a str>) -> Option<String> {
    parse_non_empty(s).map(String::from)
}

fn parse_operation_numbers<'a>(s: Option<&'a str>) -> SmallVec<[String; 2]> {
    let mut out = SmallVec::new();
    let Some(raw) = parse_non_empty(s) else {
        return out;
    };
    for item in raw.split([',', ';']) {
        let trimmed = item.trim();
        if !trimmed.is_empty() {
            out.push(String::from(trimmed));
        }
    }
    if out.is_empty() {
        out.push(String::from(raw));
    }
    out
}

pub mod operation {
    use super::BeforeAfter::{self, *};
    use super::RawOperation;
    use pest_consume::{Error, Parser, match_nodes};

    #[derive(Parser)]
    #[grammar = "operation.pest"]
    pub struct OperationParser;

    type Result<T> = std::result::Result<T, Error<Rule>>;
    type Node<'i> = pest_consume::Node<'i, Rule, ()>;

    #[pest_consume::parser]
    impl OperationParser {
        fn before(input: Node<'_>) -> Result<()> {
            return Ok(());
        }
        fn after(input: Node<'_>) -> Result<()> {
            return Ok(());
        }
        fn index(input: Node<'_>) -> Result<usize> {
            input.as_str().parse().map_err(|e| input.error(e))
        }
        fn group(input: Node<'_>) -> Result<BeforeAfter> {
            Ok(match_nodes!(input.into_children();
                [index(idx), before(_)] => B(idx),
                [index(idx), after(_)] => A(idx)
            ))
        }
        pub fn operation_key(input: Node<'_>) -> Result<impl Iterator<Item = BeforeAfter>> {
            Ok(match_nodes!(input.into_children();
                [group(groups)..] => groups
            ))
        }
        fn operation_param(input: Node<'_>) -> Result<Option<&str>> {
            let s = input.as_str();
            Ok((!s.is_empty()).then_some(s))
        }
        pub fn raw_operation(input: Node<'_>) -> Result<RawOperation<'_>> {
            let mut matched =
                match_nodes!(input.clone().into_children(); [operation_param(ops)..] => ops);
            let Some(Some(operation_type_str)) = matched.next() else {
                return Err(input.error("Operation does not have a type"));
            };
            let operation_type = operation_type_str.parse().map_err(|e| input.error(e))?;
            let mut params: [Option<&str>; 5] = [None; 5];
            for (idx, s) in matched.take(5).enumerate() {
                params[idx] = s
            }
            Ok(RawOperation {
                operation_type,
                params,
            })
        }
    }
}

pub fn parse_to_operation_hierarchy(
    input: &str,
) -> Result<impl Iterator<Item = BeforeAfter>, pest::error::Error<operation::Rule>> {
    let a = operation::OperationParser::parse(operation::Rule::operation_key, input)?.single()?;
    operation::OperationParser::operation_key(a)
}

pub fn parse_to_raw_operation<'a>(
    input: &'a str,
) -> Result<RawOperation<'a>, pest::error::Error<operation::Rule>> {
    let a = operation::OperationParser::parse(operation::Rule::raw_operation, input)?.single()?;
    operation::OperationParser::raw_operation(a)
}

#[cfg(test)]
mod test {
    use super::operation::OperationParser;
    use super::operation::Rule;
    use super::*;
    use pest_consume::Parser;
    type E = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parse_keys() -> E {
        let s = include_str!("../test/test_operations.txt");
        for input in s.lines() {
            let a = operation::OperationParser::parse(Rule::operation_key, input)?.single()?;
            let op = operation::OperationParser::operation_key(a)?.collect::<Vec<_>>();
            println!("{op:?}");
        }
        Ok(())
    }

    #[test]
    fn parse_raw_operations() -> E {
        let s = include_str!("../test/test_before.txt");
        for input in s.lines() {
            let a = operation::OperationParser::parse(Rule::raw_operation, input)?.single()?;
            let op = operation::OperationParser::raw_operation(a)?;
            println!("{op:?}");
        }
        Ok(())
    }

    #[test]
    fn comprehend_operations() -> E {
        let s = include_str!("../test/sample.oud2");
        for (key, vals) in s
            .lines()
            .filter(|e| e.starts_with("Operation"))
            .map(|s| s.split_once('=').unwrap())
        {
            if let Err(_) = OperationParser::parse(Rule::operation_key, key) {
                continue;
            };
            let vals = vals
                .split(',')
                .map(|it| {
                    OperationParser::parse(Rule::raw_operation, it)
                        .unwrap()
                        .single()
                        .unwrap()
                })
                .map(|n| OperationParser::raw_operation(n).unwrap());
            if key.ends_with('B') {
                for val in vals {
                    let b = BeforeOperation::try_from(val)?;
                    println!("{b:?}");
                }
            } else {
                for val in vals {
                    let a = AfterOperation::try_from(val)?;
                    println!("{a:?}");
                }
            }
        }
        Ok(())
    }
}
