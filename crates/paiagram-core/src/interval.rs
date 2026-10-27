use super::Interval;
use crate::{Distance, IntervalKey, NodeCollection, Wgs84LonLat};

impl Interval {
    /// The length of the interval in metres.
    ///
    /// Uses the explicitly-stored length when present; otherwise sums the
    /// great-circle (Haversine) distances between consecutive points of
    /// [`Interval::positions`].
    pub fn length(&self, key: IntervalKey, nodes: &NodeCollection) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };

        let mut total = 0.0;
        let mut prev = None;
        for pos in self.positions(key, nodes) {
            if let Some(prev) = prev {
                total += Wgs84LonLat::from(prev).distance_to_meters(Wgs84LonLat::from(pos));
            }
            prev = Some(pos);
        }
        Distance(total.round() as i32)
    }
}
