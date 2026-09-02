//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use std::io;
use std::path::PathBuf;

use crate::Command;
use crate::time::TimetableDuration;
use crate::units::time::TimetableTime;

mod oudia;
mod pyetgr;

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
pub enum ImportType {
    /// qETRC and pyETRC JSON
    Pyetgr,
    /// OuDia in Shift-JIS
    OuDia,
    /// OuDiaSecond in UTF8
    OuDiaSecond,
    /// GTFS Zip
    Gtfs,
    /// Paiagram's .paia
    PaiagramPaia,
    /// Paiagram's debug RON format
    PaiagramRon,
}

impl ImportType {
    pub fn file_extensions(&self) -> &[&'static str] {
        match self {
            Self::Pyetgr => &["json", "pyetgr"],
            Self::OuDia => &["oud"],
            Self::OuDiaSecond => &["oud2"],
            Self::Gtfs => &["zip"],
            Self::PaiagramPaia => &["paia"],
            Self::PaiagramRon => &["ron"],
        }
    }
}

pub fn generate_commands(
    data: &[u8],
    import_content: ImportType,
) -> Result<Command, Box<dyn std::error::Error>> {
    match import_content {
        ImportType::Pyetgr => {
            pyetgr::parse_pyetgr(data).ok_or(Box::new(io::Error::other("Failed!")))
        }
        ImportType::OuDia => oudia::parse_oudia(oudia::OudFileType::OuDia(data)),
        ImportType::OuDiaSecond => {
            oudia::parse_oudia(oudia::OudFileType::OuDiaSecond(&str::from_utf8(data)?))
        }
        ImportType::Gtfs => todo!(),
        ImportType::PaiagramPaia => todo!(),
        ImportType::PaiagramRon => todo!(),
    }
}
