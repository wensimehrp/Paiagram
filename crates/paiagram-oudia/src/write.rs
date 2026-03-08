use crate::Structure;

/// Recursively serializes a [`Structure`].
pub fn serialize_to<'a>(
    buf: &mut impl std::io::Write,
    values: &[Structure<'a>],
) -> std::io::Result<()> {
    for val in values.iter() {
        match val {
            Structure::Struct(key, val) => {
                buf.write_all(key.as_bytes())?;
                buf.write_all(b".\n")?;
                serialize_to(buf, val)?;
                buf.write_all(b".\n")?;
            }
            Structure::Pair(key, val) => {
                buf.write_all(key.as_bytes())?;
                buf.write_all(b"=")?;
                if let Some((first, rest)) = val.split_first() {
                    buf.write_all(first.as_bytes())?;
                    for r in rest {
                        buf.write_all(b",")?;
                        buf.write_all(r.as_bytes())?;
                    }
                }
                buf.write_all(b"\n")?;
            }
        }
    }
    Ok(())
}

/// Serializes a [`Structure`] and outputs a string
pub fn serialize_to_string<'a>(values: &[Structure<'a>]) -> std::io::Result<String> {
    let mut buf = Vec::new();
    serialize_to(&mut buf, values)?;
    String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
