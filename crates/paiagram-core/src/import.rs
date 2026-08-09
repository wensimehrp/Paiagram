//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use std::path::PathBuf;
use std::sync::Arc;

use crate::Command;
use crate::time::TimetableDuration;
use crate::units::time::{TDuration, TimetableTime};

fn normalize_times<'a>(mut time_iter: impl Iterator<Item = &'a mut TimetableTime> + 'a) {
    let Some(mut previous_time) = time_iter.next().copied() else {
        return;
    };
    for time in time_iter {
        while *time < previous_time {
            *time += TimetableDuration(86400);
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

#[derive(Clone, Copy)]
pub enum ImportContent<'a> {
    /// qETRC and pyETRC JSON
    Pyetgr(&'a str),
    /// OuDia in Shift-JIS
    OuDia(&'a [u8]),
    /// OuDiaSecond in UTF8
    OuDiaSecond(&'a str),
    /// GTFS Zip
    Gtfs(&'a [u8]),
    /// Paiagram's .paia
    PaiagramPaia(&'a [u8]),
    /// Paiagram's debug RON format
    PaiagramRon(&'a str),
}

impl<'a> ImportContent<'_> {
    fn file_extensions(&self) -> &[&'static str] {
        match self {
            Self::Pyetgr(..) => &["json", "pyetgr"],
            Self::OuDia(..) => &["oud"],
            Self::OuDiaSecond(..) => &["oud2"],
            Self::Gtfs(..) => &["zip"],
            Self::PaiagramPaia(..) => &["paia"],
            Self::PaiagramRon(..) => &["ron"],
        }
    }
}

fn read_from_file(import_content: ImportContent) -> Box<[Command]> {
    match import_content {
        ImportContent::Pyetgr(c) => todo!(),
        ImportContent::OuDia(c) => todo!(),
        ImportContent::OuDiaSecond(c) => todo!(),
        ImportContent::Gtfs(c) => todo!(),
        ImportContent::PaiagramPaia(c) => todo!(),
        ImportContent::PaiagramRon(c) => todo!(),
    }
    todo!()
}
