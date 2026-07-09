use std::borrow::Cow;

use either::Either;
use encoding_rs::SHIFT_JIS;
use paiagram_oudia::{SerializeToOud, Structure, pair, structure};
use smallvec::{SmallVec, smallvec};
use ecow::{EcoString, EcoVec};

use crate::{Key, RouteKey, StationKey, TripKey, WorldSnapshot};
use crate::trip::{TEntry, TravelMode};

fn make_disp_prop() -> Structure<'static> {
    structure!("DispProp" =>
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック;Bold=1"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック;Itaric=1"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック;Bold=1;Itaric=1"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("JikokuhyouFont"        => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("JikokuhyouVFont"       => "PointTextHeight=9;Facename=@ＭＳ ゴシック"),
        pair!("DiaEkimeiFont"         => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("DiaJikokuFont"         => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("DiaRessyaFont"         => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("CommentFont"           => "PointTextHeight=9;Facename=ＭＳ ゴシック"),
        pair!("DiaMojiColor"          => "00000000"),
        pair!("DiaHaikeiColor"        => "00FFFFFF"),
        pair!("DiaRessyaColor"        => "00000000"),
        pair!("DiaJikuColor"          => "00C0C0C0"),
        pair!("EkimeiLength"          => "6"),
        pair!("JikokuhyouRessyaWidth" => "5"),
    )
}

/// Pre-computed data for exporting a route.
pub struct RouteExportData {
    pub route_key: RouteKey,
    pub route_name: EcoString,
    pub stations: EcoVec<StationKey>,
    pub downward_trips: Vec<TripKey>,
    pub upward_trips: Vec<TripKey>,
}

pub struct OuDia<'a> {
    pub data: &'a RouteExportData,
    pub world: &'a WorldSnapshot,
}

impl<'a> super::ExportObject for OuDia<'a> {
    fn extension(&self) -> impl AsRef<str> {
        ".oud"
    }
    fn export_to_buffer(&mut self, buffer: &mut Vec<u8>) {
        let mut route_buf = vec![pair!(
            "Rosenmei" =>
            self.data.route_name.to_string()
        )];
        make_stations(&self.data.stations, self.world, &mut route_buf);
        let class_map = make_classes(self.world, &mut route_buf);
        make_diagram(self.data, self.world, &class_map, &mut route_buf);
        route_buf.extend_from_slice(&[
            pair!("KitenJikoku" => "200"),
            pair!("DiagramDgrYZahyouKyoriDefault" => "60"),
            pair!("Comment" => concat!("Exported by Paiagram ", env!("CARGO_PKG_VERSION"))),
        ]);
        let root = vec![
            pair!("FileType" => "OuDia.1.02"),
            structure!("Rosen" => ..route_buf),
            make_disp_prop(),
            pair!("FileTypeAppComment" =>
                concat!("Exported by Paiagram ", env!("CARGO_PKG_VERSION")),
            ),
        ];
        let mut utf8_buf = Vec::new();
        root.serialize_oud_to(&mut utf8_buf).unwrap();
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
    stops: &[StationKey],
    world: &WorldSnapshot,
    buf: &mut Vec<Structure<'static>>,
) {
    let Some((first, rest, last)) = split_first_middle_last(stops) else {
        return;
    };
    let make_station = |sk: StationKey, departure_display: &'static str| -> Structure<'static> {
        let name = world.stations.query(sk, |b| b.name.clone()).unwrap_or_default();
        structure!("Eki" =>
            pair!("Ekimei"           => name.to_string()),
            pair!("Ekijikokukeisiki" => departure_display),
            pair!("Ekikibo"          => "Ekikibo_Ippan"),
        )
    };

    let first_iter = std::iter::once(make_station(*first, "Jikokukeisiki_NoboriChaku"));
    let mid_iter = rest
        .iter()
        .copied()
        .map(|sk| make_station(sk, "Jikokukeisiki_Hatsu"));
    let last_iter = std::iter::once(make_station(*last, "Jikokukeisiki_KudariChaku"));
    buf.extend(first_iter);
    buf.extend(mid_iter);
    buf.extend(last_iter);
}

fn make_classes(
    world: &WorldSnapshot,
    buf: &mut Vec<Structure<'static>>,
) -> std::collections::HashMap<usize, usize> {
    let mut class_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (i, ck) in world.classes.keys().enumerate() {
        class_map.insert(ck.to_bits() as usize, i);
        let view = world.classes.get_view(*ck);
        if let Some(view) = view {
            let (r, g, b, _) = (view.style.color.r(), view.style.color.g(), view.style.color.b(), view.style.color.a());
            let color_string = format!(
                "00{:02X}{:02X}{:02X}",
                b, g, r,
            );
            buf.push(structure!("Ressyasyubetsu" =>
                pair!("Syubetsumei"         => view.name.to_string()),
                pair!("Ryakusyou"           => view.name.to_string()),
                pair!("JikokuhyouMojiColor" => color_string.clone()),
                pair!("JikokuhyouFontIndex" => "0"),
                pair!("DiagramSenColor"     => color_string),
                pair!("DiagramSenStyle"     => "SenStyle_Jissen"),
                pair!("StopMarkDrawType"    => "EStopMarkDrawType_DrawOnStop"),
            ));
        }
    }
    class_map
}

fn make_diagram(
    data: &RouteExportData,
    world: &WorldSnapshot,
    class_map: &std::collections::HashMap<usize, usize>,
    buf: &mut Vec<Structure<'static>>,
) {
    let mut dia_buf = Vec::new();
    dia_buf.push(pair!("DiaName" => "Paiagram Exported"));
    dia_buf.push(make_trainset_by_direction(
        true,
        &data.downward_trips,
        &data.stations,
        class_map,
        world,
    ));
    dia_buf.push(make_trainset_by_direction(
        false,
        &data.upward_trips,
        &data.stations,
        class_map,
        world,
    ));
    buf.push(structure!("Dia" => ..dia_buf));
}

fn make_trainset_by_direction(
    downwards: bool,
    trip_keys: &[TripKey],
    stops: &[StationKey],
    class_map: &std::collections::HashMap<usize, usize>,
    world: &WorldSnapshot,
) -> Structure<'static> {
    let format_entry = |entry: &TEntry, stn: StationKey| -> String {
        match entry {
            TEntry::Derived(s) if *s == stn => STOP.to_string(),
            TEntry::Pinned { stn: s, arr, dep, .. } if *s == stn => {
                format_pinned_time(*arr, *dep)
            }
            TEntry::PinnedNonStop { stn: s, pass, .. } if *s == stn => {
                format_pass_time(*pass)
            }
            TEntry::PinnedExternalNonStop { stn: s, pass, .. } if *s == stn => {
                format_pass_time(*pass)
            }
            _ => BYPASS.to_string(),
        }
    };

    let magic_word = if downwards { "Kudari" } else { "Nobori" };
    let mut trips = Vec::new();
    const STOP: &str = "1";
    const BYPASS: &str = "2";
    const NO_OPERATION: &str = "";

    for tk in trip_keys {
        let view = world.trips.get_view(*tk);
        if view.is_none() { continue; }
        let view = view.unwrap();

        // Find class index via class_map
        let class_idx = view.class
            .map(|ck| class_map.get(&(ck.to_bits() as usize)).copied().unwrap_or(0))
            .unwrap_or(0);

        let mut v: SmallVec<[Cow<'static, str>; 1]> = smallvec![NO_OPERATION.into(); stops.len()];
        let entries = view.schedule.entries();
        let mut next_abs_idx = 0;
        let mut stations_iter: Box<dyn Iterator<Item = &StationKey>> = if downwards {
            Box::new(stops.iter())
        } else {
            Box::new(stops.iter().rev())
        };

        for entry in entries {
            let station_key = match entry {
                TEntry::Derived(s) => *s,
                TEntry::Pinned { stn: s, .. } => *s,
                TEntry::PinnedNonStop { stn: s, .. } => *s,
                TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
                TEntry::PinnedExternal { .. } => continue,
            };

            // Advance the stations iterator
            while let Some(stn) = stations_iter.next() {
                if *stn == station_key {
                    let abs_idx = next_abs_idx;
                    v[abs_idx] = format_entry(entry, station_key).into();
                    next_abs_idx = abs_idx + 1;
                    break;
                } else {
                    // This station was skipped
                    let abs_idx = next_abs_idx;
                    v[abs_idx] = NO_OPERATION.into();
                    next_abs_idx = abs_idx + 1;
                }
            }
        }

        trips.push(structure!("Ressya" =>
            pair!("Houkou"       => magic_word),
            pair!("Syubetsu"     => class_idx.to_string()),
            pair!("Ressyabangou" => view.name.to_string()),
            pair!("EkiJikoku"    => ..v)
        ));
    }
    structure!(magic_word => ..trips)
}

fn format_pinned_time(arr: TravelMode, dep: TravelMode) -> String {
    match (arr, dep) {
        (TravelMode::At(at), TravelMode::At(dt)) => {
            let (ah, am, ..) = at.to_hmsd();
            let (dh, dm, ..) = dt.to_hmsd();
            format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
        }
        (TravelMode::At(at), TravelMode::For(d)) => {
            let (ah, am, ..) = at.to_hmsd();
            let (dh, dm, ..) = (at + d).to_hmsd();
            format!("{};{}{:02}/{}{:02}", STOP, ah, am, dh, dm)
        }
        (TravelMode::At(at), TravelMode::Flexible) => {
            let (ah, am, ..) = at.to_hmsd();
            format!("{};{}{:02}/", STOP, ah, am)
        }
        (TravelMode::Flexible, TravelMode::At(dt)) => {
            let (dh, dm, ..) = dt.to_hmsd();
            format!("{};{}{:02}", STOP, dh, dm)
        }
        (TravelMode::Flexible, TravelMode::Flexible) => STOP.to_string(),
        _ => BYPASS.to_string(),
    }
}

fn format_pass_time(pass: TravelMode) -> String {
    match pass {
        TravelMode::At(t) => {
            let (h, m, ..) = t.to_hmsd();
            format!("{};{}{:02}", BYPASS, h, m)
        }
        _ => BYPASS.to_string(),
    }
}

const STOP: &str = "1";
const BYPASS: &str = "2";
