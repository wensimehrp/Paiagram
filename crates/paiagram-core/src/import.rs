//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use std::path::PathBuf;
use std::sync::Arc;

use crate::Command;
use crate::units::time::{Duration, TimetableTime};

// mod gtfs;
// mod llt;
// mod oudia;
// mod qetrc;

fn normalize_times<'a>(mut time_iter: impl Iterator<Item = &'a mut TimetableTime> + 'a) {
    let Some(mut previous_time) = time_iter.next().copied() else {
        return;
    };
    for time in time_iter {
        while *time < previous_time {
            *time += Duration(86400);
        }
        previous_time = *time;
    }
}

fn infer_path_from_url(url: &str) -> Option<PathBuf> {
    let no_query = url.split('?').next().unwrap_or(url);
    let no_fragment = no_query.split('#').next().unwrap_or(no_query);
    let filename = no_fragment.rsplit('/').next().unwrap_or_default().trim();
    if filename.is_empty() {
        return None;
    }
    Some(PathBuf::from(filename))
}

pub enum ImportContentType {
    /// qETRC and pyETRC
    Pyetgr(Arc<str>),
    /// OuDia in Shift-JIS
    OuDia(Arc<[u8]>),
    /// OuDiaSecond in UTF8
    OuDiaSecond(Arc<str>),
    /// GTFS Zip
    Gtfs(Arc<[u8]>),
    /// Paiagram's .paia
    PaiagramPaia(Arc<str>),
    /// Paiagram's debug RON format
    PaiagramRon(Arc<str>),
}

fn load_and_trigger(path: &PathBuf, content: Vec<u8>) -> eros::Result<Box<[Command]>> {
    todo!()
}
