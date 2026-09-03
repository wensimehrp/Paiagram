use paiagram_core::{RouteKey, WorldSnapshot};
use paiagram_oudia::{OuDiaIo, Root, SerializeToOud, Structure, pair, structure};

pub struct ExportOuDia {
    world: WorldSnapshot,
    route: RouteKey,
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
        inner.serialize_oud_to(writer)
    }
}

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

fn make_root(world: &WorldSnapshot) -> Root {
    todo!()
}
