use serde::Serialize;

pub fn serialize_compressed_cbor(content: impl Serialize + Send + 'static, filename: String) {
    super::write::write_file(filename, move |writer| {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(writer);
        cbor4ii::serde::to_writer(&mut encoder, &content)
            .map_err(std::io::Error::other)
            .and_then(|_| encoder.finish().map(|_| ()).map_err(std::io::Error::other))
    });
}

struct IoToFmtWrite<'a>(&'a mut dyn std::io::Write);

impl<'a> std::fmt::Write for IoToFmtWrite<'a> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

pub fn serialize_ron(content: impl Serialize + Send + 'static, filename: String) {
    super::write::write_file(filename, move |writer| {
        ron::ser::to_writer_pretty(
            IoToFmtWrite(writer),
            &content,
            ron::ser::PrettyConfig::default(),
        )
        .map_err(std::io::Error::other)
    });
}
