use std::borrow::Cow;

use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use either::Either;
use encoding_rs::SHIFT_JIS;
use paiagram_oudia::Structure;
use paiagram_oudia::write::serialize_to;
use smallvec::{SmallVec, smallvec};

use crate::class::ClassQuery;
use crate::entry::{EntryQuery, EntryQueryItem, TravelMode};
use crate::route::{Route, RouteByDirectionTrips};
use crate::station::{ParentStationOrStation, Station};
use crate::trip::{TripQuery, TripQueryItem};

fn make_disp_prop() -> Structure<'static> {
    #[rustfmt::skip]
    let stuff: &[[&str; 2]] = &[
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック;Bold=1"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック;Itaric=1"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック;Bold=1;Itaric=1"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["JikokuhyouFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["JikokuhyouVFont", "PointTextHeight=9;Facename=@ＭＳ ゴシック"],
        ["DiaEkimeiFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["DiaJikokuFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["DiaRessyaFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["CommentFont", "PointTextHeight=9;Facename=ＭＳ ゴシック"],
        ["DiaMojiColor", "00000000"],
        ["DiaHaikeiColor", "00FFFFFF"],
        ["DiaRessyaColor", "00000000"],
        ["DiaJikuColor", "00C0C0C0"],
        ["EkimeiLength", "6"],
        ["JikokuhyouRessyaWidth", "5"],
    ];
    Structure::Struct(
        "DispProp".into(),
        stuff
            .iter()
            .copied()
            .map(|[key, val]| Structure::Pair(key.into(), smallvec![val.into()]))
            .collect(),
    )
}

pub struct OuDia<'a> {
    pub route_entity: Entity,
    pub world: &'a mut World,
}

impl<'a> super::ExportObject for OuDia<'a> {
    fn extension(&self) -> impl AsRef<str> {
        ".oud"
    }
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        let mut route_buf = vec![Structure::Pair(
            "Rosenmei".into(),
            smallvec![
                self.world
                    .get::<Name>(self.route_entity)
                    .unwrap()
                    .to_string()
                    .into()
            ],
        )];
        self.world
            .run_system_cached_with(make_stations, (self.route_entity, &mut route_buf))
            .unwrap();
        let class_map = self
            .world
            .run_system_cached_with(make_classes, &mut route_buf)
            .unwrap();
        self.world
            .run_system_cached_with(
                make_diagram,
                (&mut route_buf, self.route_entity, &class_map),
            )
            .unwrap();
        route_buf.extend_from_slice(&[
            Structure::Pair("KitenJikoku".into(), smallvec!["200".into()]),
            Structure::Pair(
                "DiagramDgrYZahyouKyoriDefault".into(),
                smallvec!["60".into()],
            ),
            Structure::Pair(
                "Comment".into(),
                smallvec![concat!("Exported by Paiagram ", env!("CARGO_PKG_VERSION")).into()],
            ),
        ]);
        let root = vec![
            Structure::Pair("FileType".into(), smallvec!["OuDia.1.02".into()]),
            Structure::Struct("Rosen".into(), route_buf),
            make_disp_prop(),
            Structure::Pair(
                "FileTypeAppComment".into(),
                smallvec![concat!("Exported by Paiagram ", env!("CARGO_PKG_VERSION")).into()],
            ),
        ];
        let mut utf8_buf = Vec::new();
        serialize_to(&mut utf8_buf, &root).unwrap();
        let s = String::from_utf8(utf8_buf).unwrap();
        // extra step: convert the buffer to shift-jis
        let (cow, _, _) = SHIFT_JIS.encode(s.as_str());
        *buffer = cow.into_owned();
    }
}

fn split_first_middle_last<T>(slice: &[T]) -> Option<(&T, &[T], &T)> {
    let (first, rest) = slice.split_first()?;
    let (last, middle) = rest.split_last().map_or((first, &[][..]), |(l, m)| (l, m));
    Some((first, middle, last))
}

fn make_stations(
    (In(route_entity), InMut(buf)): (In<Entity>, InMut<Vec<Structure<'static>>>),
    route_q: Query<&Route>,
    station_name_q: Query<&Name, With<Station>>,
) {
    let route = route_q.get(route_entity).unwrap();
    let Some((first, rest, last)) = split_first_middle_last(&route.stops) else {
        return;
    };
    let make_station = |e: Entity, departure_display: &'static str| -> Structure<'static> {
        Structure::Struct(
            "Eki".into(),
            vec![
                // 駅名
                Structure::Pair(
                    "Ekimei".into(),
                    smallvec![station_name_q.get(e).unwrap().to_string().into()],
                ),
                Structure::Pair(
                    // 駅時刻形式
                    "Ekijikokukeisiki".into(),
                    smallvec![departure_display.into()],
                ),
                // 駅規模
                Structure::Pair("Ekikibo".into(), smallvec!["Ekikibo_Ippan".into()]),
            ],
        )
    };

    let first_iter = std::iter::once(make_station(*first, "Jikokukeisiki_NoboriChaku"));
    let mid_iter = rest
        .iter()
        .copied()
        .map(|e| make_station(e, "Jikokukeisiki_Hatsu"));
    let last_iter = std::iter::once(make_station(*last, "Jikokukeisiki_KudariChaku"));
    buf.extend(first_iter);
    buf.extend(mid_iter);
    buf.extend(last_iter);
}

fn make_classes(
    InMut(buf): InMut<Vec<Structure<'static>>>,
    class_q: Query<ClassQuery>,
) -> EntityHashMap<usize> {
    let mut class_map = EntityHashMap::<usize>::new();
    let iter = class_q.iter().map(|it| {
        // ARGB
        let len = class_map.len();
        class_map.insert(it.entity, len);
        let color = it.stroke.color.get(true);
        let color_string = format!(
            "00{:02X}{:02X}{:02X}",
            // color.a(),
            color.b(),
            color.g(),
            color.r(),
        );
        Structure::Struct(
            "Ressyasyubetsu".into(),
            vec![
                Structure::Pair("Syubetsumei".into(), smallvec![it.name.to_string().into()]),
                Structure::Pair("Ryakusyou".into(), smallvec![it.name.to_string().into()]),
                Structure::Pair(
                    "JikokuhyouMojiColor".into(),
                    smallvec![color_string.clone().into()],
                ),
                Structure::Pair("JikokuhyouFontIndex".into(), smallvec!["0".into()]),
                Structure::Pair("DiagramSenColor".into(), smallvec![color_string.into()]),
                Structure::Pair(
                    "DiagramSenStyle".into(),
                    smallvec!["SenStyle_Jissen".into()],
                ),
                Structure::Pair(
                    "StopMarkDrawType".into(),
                    smallvec!["EStopMarkDrawType_DrawOnStop".into()],
                ),
            ],
        )
    });
    buf.extend(iter);
    class_map
}

fn make_diagram(
    (InMut(buf), In(route_entity), InRef(class_map)): (
        InMut<Vec<Structure<'static>>>,
        In<Entity>,
        InRef<EntityHashMap<usize>>,
    ),
    route_q: Query<(&Route, &RouteByDirectionTrips)>,
    entry_q: Query<EntryQuery>,
    trip_q: Query<TripQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
) {
    // downward: Nobori, Upward: Kudari
    let (route, RouteByDirectionTrips { downward, upward }) = route_q.get(route_entity).unwrap();
    let mut dia_buf = Vec::new();
    dia_buf.push(Structure::Pair(
        "DiaName".into(),
        smallvec!["Paiagram Exported".into()],
    ));
    dia_buf.push(make_trainset_by_direction(
        true,
        trip_q.iter_many(downward.as_slice()),
        route.stops.as_slice(),
        class_map,
        &entry_q,
        &parent_station_or_station,
    ));
    dia_buf.push(make_trainset_by_direction(
        false,
        trip_q.iter_many(upward.as_slice()),
        route.stops.as_slice(),
        class_map,
        &entry_q,
        &parent_station_or_station,
    ));
    buf.push(Structure::Struct("Dia".into(), dia_buf));
}

fn make_trainset_by_direction<'a>(
    downwards: bool,
    trips_iter: impl Iterator<Item = TripQueryItem<'a, 'a>>,
    stops: &[Entity],
    class_map: &EntityHashMap<usize>,
    entry_q: &Query<EntryQuery>,
    parent_station_or_station: &Query<ParentStationOrStation>,
) -> Structure<'static> {
    let format_time = |it: EntryQueryItem| -> String {
        match (it.mode.arr, it.mode.dep) {
            // arr at
            (Some(TravelMode::At(at)), TravelMode::At(dt)) => {
                let (ah, am, ..) = at.to_hmsd();
                let (dh, dm, ..) = dt.to_hmsd();
                format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
            }
            (Some(TravelMode::At(at)), TravelMode::For(d)) => {
                let (ah, am, ..) = at.to_hmsd();
                let (dh, dm, ..) = (at + d).to_hmsd();
                format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
            }
            (Some(TravelMode::At(at)), TravelMode::Flexible) => {
                let (ah, am, ..) = at.to_hmsd();
                format!("{};{}{:02}/", STOP, ah, am)
            }
            // arr for
            (Some(TravelMode::For(_)), TravelMode::At(dt)) => {
                let (dh, dm, ..) = dt.to_hmsd();
                let Some(e) = it.estimate else {
                    return format!("{};{}{:02}", STOP, dh, dm);
                };
                let (ah, am, ..) = e.arr.to_hmsd();
                format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
            }
            (Some(TravelMode::For(_)), TravelMode::For(_)) => {
                let Some(e) = it.estimate else {
                    return STOP.to_string();
                };
                let (ah, am, ..) = e.arr.to_hmsd();
                let (dh, dm, ..) = e.dep.to_hmsd();
                format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
            }
            (Some(TravelMode::For(_)), TravelMode::Flexible) => {
                let Some(e) = it.estimate else {
                    return STOP.to_string();
                };
                let (ah, am, ..) = e.arr.to_hmsd();
                format!("{};{}{:02}/", STOP, ah, am)
            }
            // arr flexible
            (Some(TravelMode::Flexible), TravelMode::At(t)) => {
                let (dh, dm, ..) = t.to_hmsd();
                format!("{};{}{:02}", STOP, dh, dm)
            }
            (Some(TravelMode::Flexible), TravelMode::For(_)) => {
                let Some(e) = it.estimate else {
                    return STOP.to_string();
                };
                let (ah, am, ..) = e.arr.to_hmsd();
                let (dh, dm, ..) = e.dep.to_hmsd();
                format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
            }
            (Some(TravelMode::Flexible), TravelMode::Flexible) => STOP.to_string(),
            // arr none
            (None, TravelMode::At(t)) => {
                let (h, m, ..) = t.to_hmsd();
                format!("{};{}{:02}", BYPASS, h, m)
            }
            // TODO: switch to if let guard
            (None, TravelMode::For(_)) => {
                let Some(e) = it.estimate else {
                    return BYPASS.to_string();
                };
                let (h, m, ..) = e.dep.to_hmsd();
                format!("{};{}{:02}", BYPASS, h, m)
            }
            (None, TravelMode::Flexible) => BYPASS.to_string(),
        }
    };
    let magic_word = if downwards { "Kudari" } else { "Nobori" };
    let mut trips = Vec::new();
    const STOP: &str = "1";
    const BYPASS: &str = "2";
    const NO_OPERATION: &str = "";
    for it in trips_iter {
        let a = class_map.get(&it.class.entity());
        let mut entries = vec![
            Structure::Pair("Houkou".into(), smallvec![magic_word.into()]),
            Structure::Pair("Syubetsu".into(), smallvec![a.unwrap().to_string().into()]),
            Structure::Pair("Ressyabangou".into(), smallvec![it.name.to_string().into()]),
        ];
        let mut v: SmallVec<[Cow<'static, str>; 1]> = smallvec![NO_OPERATION.into(); stops.len()];
        let schedule_it = entry_q.iter_many(it.schedule.iter());
        let mut next_abs_idx = 0;
        let mut stations = if downwards {
            Either::Left(stops.iter())
        } else {
            Either::Right(stops.iter().rev())
        };
        for it in schedule_it {
            let station_entity = parent_station_or_station.get(it.stop()).unwrap().parent();
            // we reuse the same iterator here
            // the pointer would advance every time we use the .position() method
            if let Some(found_pos) = stations.position(|it| *it == station_entity) {
                let abs_idx = next_abs_idx + found_pos;
                v[abs_idx] = format_time(it).into();
                next_abs_idx = abs_idx + 1;
            }
        }
        entries.push(Structure::Pair("EkiJikoku".into(), v));
        trips.push(Structure::Struct("Ressya".into(), entries));
    }
    Structure::Struct(magic_word.into(), trips)
}
