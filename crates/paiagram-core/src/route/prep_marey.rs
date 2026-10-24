use std::fmt::Write;
use std::iter;

use csv::Writer;
use ecow::{EcoVec, eco_vec};
use paiagram_rw::ExportObject;

use crate::route::{MareyCache, StationRecord};
use crate::trip::{EstimateEntry, TEstimate};
use crate::{TripKey, WorldSnapshot};

#[derive(Clone, Copy)]
pub enum RouteTimetableEntry {
    NotInOperation,
    Active((TEstimate, EstimateEntry)),
}

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq)]
    pub struct StationDisplayMode: u8 {
        const ARRIVAL   = 1 << 0;
        const TRACK     = 1 << 1;
        const DEPARTURE = 1 << 2;
    }
}

/// The timetable to be displayed
#[derive(Default, Clone)]
pub struct RouteTimetable {
    /// A list of stations to display
    pub station_records: EcoVec<(StationRecord, StationDisplayMode, StationDisplayMode)>,
    /// Inbound trips, i.e. downward trips. This is a sorted EcoVec
    /// where each element is sorted by first [`RouteTimetableEntry::Active`] index
    /// and time
    pub inbound_columns: EcoVec<(TripKey, EcoVec<RouteTimetableEntry>)>,
    /// Outbound trips, i.e. upward trips (but outbound trips is displayed downwards).
    /// Each element is sorted by last [`RouteTimetableEntry::Active`] index
    /// and time
    pub outbound_columns: EcoVec<(TripKey, EcoVec<RouteTimetableEntry>)>,
}

/// Merge new column cells into the old column.
/// Returns the updated column if there aren't any overlapping cells.
/// Returns None if cells overlap.
fn merge_candidates(
    mut old: EcoVec<RouteTimetableEntry>,
    range: std::ops::Range<usize>,
    new_items: impl Iterator<Item = RouteTimetableEntry> + Clone,
) -> Option<EcoVec<RouteTimetableEntry>> {
    old.extend(iter::repeat_n(
        RouteTimetableEntry::NotInOperation,
        range.end.saturating_sub(old.len()),
    ));
    for (old_ref, new_item) in iter::zip(
        old.make_mut().iter_mut().skip(range.start).take(range.clone().count()),
        new_items.clone(),
    ) {
        if !matches!(old_ref, RouteTimetableEntry::NotInOperation) {
            return None;
        }
        *old_ref = new_item;
    }
    Some(old)
}

fn merge_or_push_columns(
    mut columns: EcoVec<(TripKey, EcoVec<RouteTimetableEntry>)>,
    new_column: &[(TEstimate, EstimateEntry, u32, f32)],
    trip_key: TripKey,
    is_inbound: bool,
    station_count: usize,
) -> EcoVec<(TripKey, EcoVec<RouteTimetableEntry>)> {
    let first_idx = new_column.first().unwrap().2 as usize;
    let start_idx = if is_inbound {
        first_idx
    } else {
        station_count - 1 - first_idx
    };
    let new_entries: EcoVec<RouteTimetableEntry> = new_column
        .iter()
        .map(|&(es, entry, ..)| RouteTimetableEntry::Active((es, entry)))
        .collect();

    // try to merge into the last column
    if let Some((prev_key, prev_segment)) = columns.make_mut().last_mut()
        && *prev_key == trip_key
        && let Some(merged) = merge_candidates(
            prev_segment.clone(),
            start_idx..(start_idx + new_column.len()),
            new_entries.iter().copied(),
        )
    {
        *prev_segment = merged;
    } else {
        // create a new column if merge failed
        let mut col = eco_vec![RouteTimetableEntry::NotInOperation; start_idx];

        col.extend(new_entries);
        columns.push((trip_key, col));
    }
    columns
}

impl RouteTimetable {
    pub fn new<'a>(
        marey_cache: MareyCache,
        station_records: impl Iterator<Item = &'a StationRecord>,
    ) -> Self {
        let station_records: EcoVec<_> = station_records
            .cloned()
            .map(|it| {
                (
                    it,
                    StationDisplayMode::DEPARTURE,
                    StationDisplayMode::DEPARTURE,
                )
            })
            .collect();
        let station_count = station_records.len();
        let (inbound_columns, outbound_columns) = marey_cache.map.into_iter().fold(
            (
                EcoVec::<(TripKey, EcoVec<RouteTimetableEntry>)>::new(),
                EcoVec::<(TripKey, EcoVec<RouteTimetableEntry>)>::new(),
            ),
            |(inbound_columns, outbound_columns), (trip_key, segments)| {
                let new_inbound_columns = segments
                    .iter()
                    .flat_map(|segment| segment.chunk_by(|prev, curr| prev.2 < curr.2))
                    .filter(|segment| segment.len() >= 2);
                let new_outbound_columns = segments
                    .iter()
                    .flat_map(|segment| segment.chunk_by(|prev, curr| prev.2 > curr.2))
                    .filter(|segment| segment.len() >= 2);
                (
                    new_inbound_columns.fold(inbound_columns, |acc, new_col| {
                        merge_or_push_columns(acc, new_col, trip_key, true, station_count)
                    }),
                    new_outbound_columns.fold(outbound_columns, |acc, new_col| {
                        merge_or_push_columns(acc, new_col, trip_key, false, station_count)
                    }),
                )
            },
        );
        Self {
            station_records,
            inbound_columns,
            outbound_columns,
        }
    }

    // TODO: handle outbound export
    fn to_csv_string(&self, snap: &WorldSnapshot) -> Result<String, Box<dyn std::error::Error>> {
        let mut writer = Writer::from_writer(vec![]);

        // header
        writer.write_field("Trips")?;
        for s in self
            .station_records
            .iter()
            .map(|(record, _, _)| {
                record.station_key(&snap.graph.nodes).and_then(|key| snap.stations.get(key))
            })
            .map(|wfc| wfc.map_or_default(|wfc| wfc.name.as_str()))
        {
            writer.write_field(s)?;
        }
        writer.write_record(None::<&[u8]>)?;

        // trips
        let mut entry_fmt_buf = String::with_capacity(24);
        for (trip_key, times) in &self.inbound_columns {
            let trip_name = snap.trips.get(trip_key).map_or_default(|wfc| wfc.name.as_str());
            writer.write_field(trip_name)?;
            let mut written = 0;
            for ((_, stn_display, _), entry) in std::iter::zip(&self.station_records, times) {
                written += 1;
                entry_fmt_buf.clear();
                if stn_display.contains(StationDisplayMode::ARRIVAL) {
                    match entry {
                        RouteTimetableEntry::NotInOperation => write!(entry_fmt_buf, "arr:..")?,
                        RouteTimetableEntry::Active((estimate, _)) => {
                            write!(entry_fmt_buf, "arr:{}", estimate.arr)?
                        }
                    }
                }
                if !entry_fmt_buf.is_empty() {
                    write!(entry_fmt_buf, "|")?;
                }
                if stn_display.contains(StationDisplayMode::DEPARTURE) {
                    match entry {
                        RouteTimetableEntry::NotInOperation => write!(entry_fmt_buf, "dep:..")?,
                        RouteTimetableEntry::Active((estimate, _)) => {
                            write!(entry_fmt_buf, "dep:{}", estimate.dep)?
                        }
                    }
                }
                writer.write_field(entry_fmt_buf.as_str())?;
            }
            // A trip can end before the last station, leaving the row shorter than the header.
            // `csv::Writer` rejects records whose length differs from the first one, so pad it.
            for _ in written..self.station_records.len() {
                writer.write_field("")?;
            }
            writer.write_record(None::<&[u8]>)?;
        }

        // convert
        let data = String::from_utf8(writer.into_inner()?)?;
        Ok(data)
    }
}

pub struct RouteTimetableExport(pub RouteTimetable, pub WorldSnapshot);

impl ExportObject for RouteTimetableExport {
    fn filename(&self) -> String {
        "exported.csv".into()
    }
    fn write_content<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let csv =
            self.0.to_csv_string(&self.1).map_err(|e| std::io::Error::other(e.to_string()))?;
        writer.write_all(csv.as_bytes())
    }
}
