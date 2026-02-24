//! # Import
//! Handles foreign formats such as GTFS Static, qETRC/pyETRC, and OuDiaSecond.

use std::path::PathBuf;

use crate::{
    graph::Graph,
    interval::Interval,
    rw::save::{LoadCandidate, SaveData},
    station::Station,
    trip::class::{Class, ClassBundle},
    units::{
        distance::Distance,
        time::{Duration, TimetableTime},
    },
};
use anyhow::{Result, anyhow};
use bevy::{
    platform::collections::HashMap,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future::poll_once},
};
use moonshine_core::kind::*;

mod gtfs;
mod oudia;
mod qetrc;

pub struct ImportPlugin;
impl Plugin for ImportPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(qetrc::load_qetrc)
            .add_observer(oudia::load_oud)
            .add_observer(gtfs::load_gtfs_static)
            .add_observer(download_file)
            .add_systems(Update, pull_file);
    }
}

#[derive(Event)]
pub struct LoadQETRC {
    pub content: String,
}

pub enum OuDiaContentType {
    OuDiaSecond(String),
    OuDia(Vec<u8>),
}

#[derive(Event)]
pub struct LoadOuDia {
    pub content: OuDiaContentType,
}

impl LoadOuDia {
    pub fn original(data: Vec<u8>) -> Self {
        Self {
            content: OuDiaContentType::OuDia(data),
        }
    }
    pub fn second(data: String) -> Self {
        Self {
            content: OuDiaContentType::OuDiaSecond(data),
        }
    }
}

#[derive(Event)]
pub struct LoadGTFS {
    pub content: Vec<u8>,
}

#[derive(Event)]
pub struct DownloadFile {
    pub url: String,
}

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

pub(crate) fn make_station(
    name: &str,
    station_map: &mut HashMap<String, Instance<Station>>,
    graph: &mut Graph,
    commands: &mut Commands,
) -> Instance<Station> {
    if let Some(&entity) = station_map.get(name) {
        return entity;
    }
    let station_entity = commands
        .spawn(Name::new(name.to_string()))
        .insert_instance(Station::default())
        .into();
    station_map.insert(name.to_string(), station_entity);
    graph.add_node(station_entity.entity());
    station_entity
}

pub(crate) fn make_class(
    name: &str,
    class_map: &mut HashMap<String, Instance<Class>>,
    commands: &mut Commands,
    mut make_class: impl FnMut() -> ClassBundle,
) -> Instance<Class> {
    if let Some(&entity) = class_map.get(name) {
        return entity;
    };
    let class_bundle = make_class();
    let class_entity = commands
        .spawn((class_bundle.name, class_bundle.stroke))
        .insert_instance(class_bundle.class)
        .into();
    class_map.insert(name.to_string(), class_entity);
    class_entity
}

pub(crate) fn add_interval_pair(
    graph: &mut Graph,
    commands: &mut Commands,
    from: Entity,
    to: Entity,
    length: Distance,
) {
    if graph.contains_edge(from, to) || graph.contains_edge(to, from) {
        return;
    }
    let e1: Instance<Interval> = commands.spawn_instance(Interval { length }).into();
    let e2: Instance<Interval> = commands.spawn_instance(Interval { length }).into();
    graph.add_edge(from, to, e1.entity());
    graph.add_edge(to, from, e2.entity());
}

#[derive(Component)]
pub struct FileDownloadTask {
    task: Option<Task<(Vec<u8>, String)>>,
    url: String,
}

pub fn download_file(event: On<DownloadFile>, mut commands: Commands) {
    commands.spawn(FileDownloadTask {
        task: None,
        url: event.url.clone(),
    });
}

fn pull_file(mut commands: Commands, tasks: Populated<(Entity, &mut FileDownloadTask)>) {
    for (task_entity, mut task) in tasks {
        if task.task.is_none() {
            let url = task.url.clone();
            task.task = Some(AsyncComputeTaskPool::get().spawn(async move {
                let response = ehttp::fetch_async(ehttp::Request::get(&url))
                    .await
                    .unwrap_or_else(|e| panic!("Failed to download file from {url}: {e:?}"));
                if !response.ok {
                    panic!(
                        "Failed to download file from {url}: status={} {}",
                        response.status, response.status_text
                    );
                }
                (response.bytes, response.url)
            }));
            continue;
        }

        let Some(task_handle) = task.task.as_mut() else {
            continue;
        };
        let Some((content, final_url)) = block_on(poll_once(task_handle)) else {
            continue;
        };

        let path = infer_path_from_url(&final_url)
            .or_else(|| infer_path_from_url(&task.url))
            .unwrap_or_else(|| PathBuf::from(task.url.clone()));
        if let Err(e) = load_and_trigger(&path, content, &mut commands) {
            error!(
                "Failed to load downloaded file from {} (resolved as {}): {e:#}",
                task.url,
                path.display(),
            );
        }
        commands.entity(task_entity).despawn();
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

pub fn load_and_trigger(path: &PathBuf, content: Vec<u8>, commands: &mut Commands) -> Result<()> {
    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    match path.extension().and_then(|s| s.to_str()) {
        Some("pyetgr") | Some("json") => {
            let content = String::from_utf8(content)?;
            commands.trigger(LoadQETRC { content });
        }
        Some("oud2") => {
            let content = String::from_utf8(content)?;
            commands.trigger(LoadOuDia::second(content));
        }
        Some("zip") => {
            commands.trigger(LoadGTFS { content });
        }
        Some("oud") => {
            // oudia does not use utf-8
            commands.trigger(LoadOuDia::original(content))
        }
        Some("lz4") | Some("txt") if filename.ends_with(".lz4.txt") => {
            commands.insert_resource(LoadCandidate(SaveData::CompressedCbor(content)));
        }
        Some("ron") | Some("txt") if filename.ends_with(".ron.txt") => {
            commands.insert_resource(LoadCandidate(SaveData::Ron(content)));
        }
        Some(e) => return Err(anyhow!("Unexpected extension: {e}")),
        None => return Err(anyhow!("Path does not have an extension")),
    }
    return Ok(());
}
