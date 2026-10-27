use std::io::{self, Write};

use paiagram_core::time::{TTime, TimetableTime};
use paiagram_core::trip::{EstimateEntry, TravelMode};
use paiagram_core::{Key, NodeKey, ServiceClassKey, StationKey, TripKey, Wgs84LonLat, World};
use paiagram_rw::ExportObject;
use zip::ZipWriter;
use zip::write::FileOptions;

/// The only service this exporter knows about: every trip runs every day.
const SERVICE_ID: &str = "daily";
const AGENCY_ID: &str = "paiagram";
/// GTFS `route_type` 2 is "Rail". Paiagram does not store the mode, so this is a default.
const ROUTE_TYPE: &str = "2";
const ROUTE_UNKNOWN_ID: &str = "route_unknown";
const CALENDAR_START_DATE: &str = "20000101";
const CALENDAR_END_DATE: &str = "21001231";

pub struct ExportGtfs {
    pub world: World,
}

impl ExportObject for ExportGtfs {
    fn filename(&self) -> String {
        "exported.zip".into()
    }

    fn write_content<W: io::Write + io::Seek>(&mut self, writer: &mut W) -> io::Result<()> {
        let mut writer = ZipWriter::new(writer);
        write_agency(&mut writer)?;
        write_stops(&mut writer, &self.world)?;
        write_routes(&mut writer, &self.world)?;
        write_trips(&mut writer, &self.world)?;
        write_stop_times(&mut writer, &self.world)?;
        write_calendar(&mut writer)?;
        write_feed_info(&mut writer)?;
        writer.finish()?;
        Ok(())
    }
}

fn write_agency<W: Write + io::Seek>(writer: &mut ZipWriter<W>) -> io::Result<()> {
    writer.start_file("agency.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record(["agency_id", "agency_name", "agency_url", "agency_timezone"])?;
    csv.write_record([AGENCY_ID, "Paiagram", "https://paiagram.com", "Etc/UTC"])?;
    csv.flush()?;
    Ok(())
}

fn write_stops<W: Write + io::Seek>(writer: &mut ZipWriter<W>, world: &World) -> io::Result<()> {
    writer.start_file("stops.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record([
        "stop_id",
        "stop_name",
        "stop_lat",
        "stop_lon",
        "location_type",
        "parent_station",
    ])?;

    // A Paiagram station becomes a GTFS parent station...
    for (key, station) in &world.stations {
        let pos = Wgs84LonLat::from(station.pos);
        csv.write_record([
            station_id(*key),
            station.name.to_string(),
            format!("{:.7}", pos.lat),
            format!("{:.7}", pos.lon),
            "1".to_string(),
            String::new(),
        ])?;
    }

    // ...and each of its nodes becomes a platform/stop below it.
    for (key, node) in world.graph.nodes() {
        let pos = Wgs84LonLat::from(node.pos);
        csv.write_record([
            node_id(*key),
            node.name.to_string(),
            format!("{:.7}", pos.lat),
            format!("{:.7}", pos.lon),
            "0".to_string(),
            station_id(node.parent),
        ])?;
    }

    csv.flush()?;
    Ok(())
}

fn write_routes<W: Write + io::Seek>(writer: &mut ZipWriter<W>, world: &World) -> io::Result<()> {
    writer.start_file("routes.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record([
        "route_id",
        "agency_id",
        "route_short_name",
        "route_long_name",
        "route_type",
        "route_color",
    ])?;

    for (key, class) in &world.service_classes {
        let [r, g, b, _] = class.style.color.to_srgba_unmultiplied();
        csv.write_record([
            service_class_id(*key),
            AGENCY_ID.to_string(),
            class.name.to_string(),
            String::new(),
            ROUTE_TYPE.to_string(),
            format!("{r:02X}{g:02X}{b:02X}"),
        ])?;
    }

    // Trips without a service class still need a valid `route_id`.
    if world.trips.iter().any(|(_, trip)| trip.service_class.is_none()) {
        csv.write_record([
            ROUTE_UNKNOWN_ID.to_string(),
            AGENCY_ID.to_string(),
            "Unknown".to_string(),
            String::new(),
            ROUTE_TYPE.to_string(),
            "808080".to_string(),
        ])?;
    }

    csv.flush()?;
    Ok(())
}

fn write_trips<W: Write + io::Seek>(writer: &mut ZipWriter<W>, world: &World) -> io::Result<()> {
    writer.start_file("trips.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record([
        "route_id",
        "service_id",
        "trip_id",
        "trip_short_name",
        "trip_headsign",
    ])?;

    for (key, trip) in &world.trips {
        let route_id = trip
            .service_class
            .map(service_class_id)
            .unwrap_or_else(|| ROUTE_UNKNOWN_ID.to_string());
        let headsign = trip
            .schedule
            .entries()
            .last()
            .and_then(|entry| world.graph.nodes().get(&entry.node))
            .map_or_else(String::new, |node| node.name.to_string());
        csv.write_record([
            route_id,
            SERVICE_ID.to_string(),
            trip_id(*key),
            trip.name.to_string(),
            headsign,
        ])?;
    }

    csv.flush()?;
    Ok(())
}

fn write_stop_times<W: Write + io::Seek>(
    writer: &mut ZipWriter<W>,
    world: &World,
) -> io::Result<()> {
    writer.start_file("stop_times.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record([
        "trip_id",
        "arrival_time",
        "departure_time",
        "stop_id",
        "stop_sequence",
    ])?;

    for (key, trip) in &world.trips {
        let trip_id = trip_id(*key);
        // `estimates` also contains derived nodes along the graph path between two
        // scheduled stops. Only pinned entries are actual stops of the trip.
        let estimates = trip.schedule.estimates(&world.graph, |estimates| estimates.to_vec());
        let mut stop_sequence = 0usize;
        for (estimate, entry) in estimates {
            let EstimateEntry::Pinned(entry) = entry else {
                continue;
            };
            let (arr, dep) = match estimate {
                Some(estimate) => (Some(estimate.arr), Some(estimate.dep)),
                None => entry_times(&entry),
            };
            let (Some(arr), Some(dep)) = (arr, dep) else {
                continue;
            };
            csv.write_record([
                trip_id.clone(),
                format_time(arr),
                format_time(dep),
                node_id(entry.node),
                stop_sequence.to_string(),
            ])?;
            stop_sequence += 1;
        }
    }

    csv.flush()?;
    Ok(())
}

fn write_calendar<W: Write + io::Seek>(writer: &mut ZipWriter<W>) -> io::Result<()> {
    writer.start_file("calendar.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record([
        "service_id",
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
        "start_date",
        "end_date",
    ])?;
    csv.write_record([
        SERVICE_ID,
        "1",
        "1",
        "1",
        "1",
        "1",
        "1",
        "1",
        CALENDAR_START_DATE,
        CALENDAR_END_DATE,
    ])?;
    csv.flush()?;
    Ok(())
}

fn write_feed_info<W: Write + io::Seek>(writer: &mut ZipWriter<W>) -> io::Result<()> {
    writer.start_file("feed_info.txt", FileOptions::DEFAULT)?;
    let mut csv = csv::Writer::from_writer(writer);
    csv.write_record([
        "feed_publisher_name",
        "feed_publisher_url",
        "feed_lang",
        "feed_start_date",
        "feed_end_date",
        "feed_version",
    ])?;
    csv.write_record([
        "Paiagram",
        "https://paiagram.com",
        "en",
        CALENDAR_START_DATE,
        CALENDAR_END_DATE,
        env!("CARGO_PKG_VERSION"),
    ])?;
    csv.flush()?;
    Ok(())
}

/// Fall back to the literal travel modes when the estimate could not be resolved (e.g. a trip
/// made entirely of flexible times).
fn entry_times(entry: &paiagram_core::trip::TEntry) -> (Option<TTime>, Option<TTime>) {
    let arr = match entry.arr_or_pass {
        TravelMode::At(time) => Some(time),
        TravelMode::For(_) | TravelMode::Flexible => None,
    };
    let dep = match entry.dep {
        Some(TravelMode::At(time)) => Some(time),
        Some(TravelMode::For(_) | TravelMode::Flexible) | None => None,
    };
    // GTFS needs both columns; a stop with only one of them known uses it for both.
    let arr = arr.or(dep);
    let dep = dep.or(arr);
    (arr, dep)
}

/// GTFS times are `HH:MM:SS` and may exceed 24 hours for trips past midnight.
fn format_time(time: TimetableTime) -> String {
    let seconds = time.0.max(0);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

fn station_id(key: StationKey) -> String {
    format!("station_{:x}", key.to_bits())
}

fn node_id(key: NodeKey) -> String {
    format!("stop_{:x}", key.to_bits())
}

fn service_class_id(key: ServiceClassKey) -> String {
    format!("route_{:x}", key.to_bits())
}

fn trip_id(key: TripKey) -> String {
    format!("trip_{:x}", key.to_bits())
}
