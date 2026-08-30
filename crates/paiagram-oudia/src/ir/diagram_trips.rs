use either::Either;

use super::{Diagram, Station};
use crate::{Direction, ServiceMode, TimetableEntry, Trip};

impl Diagram {
    /// Given a list of stations, return the list of trips with their entries
    /// deduplicated and blah blah blah, to flight around OuDiaSecond quirks.
    pub fn trip_station_times<'a, 'stn>(
        &'a self,
        stations: &'stn [&'stn Station],
    ) -> impl Iterator<
        Item = (
            &'stn Trip,
            impl Iterator<Item = (&'stn Station, &'stn TimetableEntry)>,
        ),
    >
    where
        'a: 'stn,
    {
        self.trips.iter().map(move |tr| {
            let stn_iter = match tr.direction {
                Direction::Down => Either::Left(stations.iter().copied()),
                Direction::Up => Either::Right(stations.iter().rev().copied()),
            };
            (
                tr,
                stn_iter
                    .zip(tr.times.iter())
                    .filter(|(_stn, ent)| ent.service_mode != ServiceMode::NoOperation),
            )
        })
    }
}

#[cfg(test)]
mod test {
    use crate::ir::StationToGraph;
    use crate::ir::test::get_ir_small;
    #[test]
    fn trip_station_times() {
        let ir = get_ir_small().unwrap();
        let deduplicated = ir.route.stations.merge_duplicate();
        for (trip, it) in ir.route.diagrams[0].trip_station_times(&deduplicated) {
            println!("{:?} -> {:?}", trip.name, trip.direction);
            for (stn, time) in it {
                let width = 16 - stn.name.chars().count() * 2;
                println!(
                    "    {}{:width$}: {:16?}, {:16?}",
                    stn.name, "", time.arrival_time, time.departure_time
                );
            }
        }
    }
}
