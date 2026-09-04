use paiagram_core::{RouteKey, WorldSnapshot};
use paiagram_oudia::{OuDiaIo, Root, Route, SerializeToOud, Structure, Time as OudTime};

pub struct ExportOuDia {
    pub world: WorldSnapshot,
    pub route: RouteKey,
    pub is_oudia_second: bool,
}

impl paiagram_rw::ExportObject for ExportOuDia {
    fn extension(&self) -> impl AsRef<str> {
        ".oud"
    }
    fn write_content<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let root = make_root(&self.world);
        let Structure::Struct(_, inner) = root.to_structure() else {
            unreachable!();
        };
        if self.is_oudia_second {
            // UTF-8
            inner.serialize_oud_to(writer)
        } else {
            // Doesn't have UTF-8
            writer.write_all(&inner.to_shift_jis_string()?)
        }
    }
}

fn make_root(world: &WorldSnapshot) -> Root {
    Root {
        file_type: "this".into(),
        file_type_app_comment: None,
        route: Route {
            name: "that".into(),
            stations: Vec::new(),
            classes: Vec::new(),
            display_start_time: OudTime::from_hms(4, 0, 0),
            diagrams: Vec::new(),
            comment: concat!("Exported by Paiagram ", env!("CARGO_PKG_VERSION")).into(),
            down_dia_alias: None,
            up_dia_alias: None,
            diagram_station_interval_default: 0,
            enable_operation: None,
            operation_number_reverse: None,
            operation_crosses_start_time: None,
            reference_diagram_index: None,
            disable_hidden_class: None,
        },
        display_properties: Default::default(),
        window_position: Default::default(),
    }
}
