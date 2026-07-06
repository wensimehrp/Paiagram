//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use std::path::Path;
use std::sync::Arc;

use crate::Command;
use crate::units::time::{Duration, TimetableTime};

pub mod oudia;
pub mod gtfs;
pub mod llt;
pub mod qetrc;

pub fn normalize_times<'a>(time_iter: impl Iterator<Item = &'a mut TimetableTime>) {
    let mut iter = time_iter.peekable();
    let Some(first) = iter.next() else {
        return;
    };
    let mut previous_time = *first;
    for time in iter {
        while *time < previous_time {
            *time += Duration(86400);
        }
        previous_time = *time;
    }
}

fn infer_path_from_url(url: &str) -> Option<std::path::PathBuf> {
    let no_query = url.split('?').next().unwrap_or(url);
    let no_fragment = no_query.split('#').next().unwrap_or(no_query);
    let filename = no_fragment.rsplit('/').next().unwrap_or_default().trim();
    if filename.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(filename))
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

pub fn load_and_trigger(path: &Path, content: Vec<u8>) -> Result<Box<[Command]>, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let commands: Vec<Command> = match ext.as_str() {
        "oud" => oudia::parse_oud(&content)?,
        "oud2" => {
            let s = String::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            oudia::parse_oud2(&s)?
        }
        "pyetgr" | "json" => {
            let s = String::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            qetrc::load_qetrc(&s)?
        }
        "zip" => {
            gtfs::load_gtfs_static(&content)?
        }
        "paia" => {
            let _s = String::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            return Err("Paiagram save import not yet implemented".into());
        }
        "ron" => {
            let _s = String::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            return Err("RON import not yet implemented".into());
        }
        _ => return Err(format!("Unknown file extension: {ext}")),
    };
    Ok(commands.into_boxed_slice())
}
