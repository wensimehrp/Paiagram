use super::Interval;
use crate::{Distance, Wgs84LonLat};

impl Interval {
    /// The length of the interval in metres.
    ///
    /// Uses the explicitly-stored length when present; otherwise sums the
    /// great-circle (Haversine) distances between consecutive points in
    /// [`Interval::nodes`].
    pub fn length(&self) -> Distance {
        if let Some(d) = self.length {
            return Distance(d.get() as i32);
        };
        let total: f64 = self
            .nodes
            .windows(2)
            .map(|w| Wgs84LonLat::from(w[0]).distance_to_meters(Wgs84LonLat::from(w[1])))
            .sum();
        Distance(total.round() as i32)
    }
}
