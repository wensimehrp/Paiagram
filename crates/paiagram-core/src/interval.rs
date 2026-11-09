use super::Interval;
use crate::graph::Graph;
use crate::{Coord, Distance, IntervalKey};

impl Interval {
    /// The length of the interval in metres.
    ///
    /// Uses the explicitly-stored length when present; otherwise sums the distances between
    /// consecutive points of [`Interval::positions`] according to the graph's coordinate type.
    pub fn length(&self, key: IntervalKey, graph: &Graph) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };

        let mut total = 0.0;
        let mut prev: Option<Coord> = None;
        for pos in self.positions(key, graph.nodes()) {
            if let Some(prev) = prev {
                total += graph.coord_type.distance_to_meters(prev, pos);
            }
            prev = Some(pos);
        }
        Distance(total.round() as i32)
    }
}
