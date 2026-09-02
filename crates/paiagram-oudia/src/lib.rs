// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(docsrs, feature(doc_cfg))]
/*!
A utility crate for parsing the .oud/.oud2 formats used by timetabling tools
[OuDia](https://web.archive.org/web/20240909024820/https://take-okm.a.la9.jp/oudia/index.html)
and [OuDiaSecond](http://oudiasecond.seesaa.net/). This crate does not support
parsing WINDIA files.

This parses .oud/.oud2 strings into human readable intermediate
representation in plain, comprehensible English (as in the [`ir`] module).
The crate's goal is to provide a friendly interface for interacting with those
formats. There crate also provides serialization support from AST to .oud/.oud2
structure.

# Getting Started

To get started, simply use [`parse_oud2_to_ir`] for .oud2, or [`parse_oud_to_ir`]
for .oud.

Alternatively, you can use [`parse_to_ast`] if you want to parse a file to AST and
interact with the AST directly.
*/
pub use ast::{SerializeToOud, Structure, parse_to_ast};
pub use ir::*;
#[doc(hidden)]
pub use smallvec;
pub use time::Time;
pub use timetable::{ServiceMode, TimetableEntry};

mod ir_macros;

pub mod ast;
pub mod ir;
pub mod operation;
pub mod time;
pub mod timetable;

#[macro_export]
macro_rules! structure {
    // start recursive accumulation
    ($k:expr => $($tokens:tt)*) => {{
        let mut items = Vec::new();
        $crate::structure!(@extend items $($tokens)*);
        $crate::Structure::Struct($k.into(), items)
    }};

    // handle the ".." syntax for iterators
    (@extend $items:ident .. $x:expr, $($rest:tt)*) => {
        $items.extend($x.into_iter().map(|i| i.into()));
        $crate::structure!(@extend $items $($rest)*);
    };

    // handle the ".." syntax for the final item w/ no trailing comma
    (@extend $items:ident .. $x:expr) => {
        $items.extend($x.into_iter().map(|i| i.into()));
    };

    // handle a single expression
    (@extend $items:ident $x:expr, $($rest:tt)*) => {
        $items.push($x.into());
        $crate::structure!(@extend $items $($rest)*);
    };

    // handle the final single expression w/ no trailing comma
    (@extend $items:ident $x:expr) => {
        $items.push($x.into());
    };

    // stop when no tokens are left
    (@extend $items:ident $(,)?) => {};
}

#[macro_export]
macro_rules! pair {
    // start recursive accumulation
    ($k:expr => $($tokens:tt)*) => {{
        let mut items = $crate::smallvec::SmallVec::new();
        $crate::pair!(@extend items $($tokens)*);
        $crate::Structure::Pair($k.into(), items)
    }};

    // handle the ".." syntax for iterators
    (@extend $items:ident .. $x:expr, $($rest:tt)*) => {
        $items.extend($x.into_iter().map(|i| i.into()));
        $crate::pair!(@extend $items $($rest)*);
    };

    // handle the ".." syntax for the final item w/ no trailing comma
    (@extend $items:ident .. $x:expr) => {
        $items.extend($x.into_iter().map(|i| i.into()));
    };

    // handle a single expression
    (@extend $items:ident $x:expr, $($rest:tt)*) => {
        $items.push($x.into());
        $crate::pair!(@extend $items $($rest)*);
    };

    // handle the final single expression w/ no trailing comma
    (@extend $items:ident $x:expr) => {
        $items.push($x.into());
    };

    // stop when no tokens are left
    (@extend $items:ident $(,)?) => {};
}

/// Parse a UTF-8 encoded Oud2 string slice into a [`Root`] intermediate representation.
pub fn parse_oud2_to_ir(input: &str) -> Result<Root, IrConversionError> {
    let v = parse_to_ast(input).map_err(IrConversionError::from)?;
    Root::try_from(v.as_slice())
}

/// Parse a Shift-JIS encoded Oud slice into a [`Root`] intermediate representation.
pub fn parse_oud_to_ir(input: &[u8]) -> Result<Root, IrConversionError> {
    let (utf_8_input, _, _) = encoding_rs::SHIFT_JIS.decode(input);
    let v = parse_to_ast(&utf_8_input).map_err(IrConversionError::from)?;
    Root::try_from(v.as_slice())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    type E = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parse_all_files_to_ir() -> E {
        let mut dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        dir_path.push("test");
        let entries = std::fs::read_dir(dir_path)?;
        for entry in entries {
            let path = entry?.path();
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str());
                match ext {
                    Some("oud") => {
                        let content = std::fs::read(&path)?;
                        parse_oud_to_ir(&content)?;
                    }
                    Some("oud2") => {
                        let content = std::fs::read_to_string(&path)?;
                        parse_oud2_to_ir(&content)?;
                    }
                    // Ignore everything else
                    _ => continue,
                }
            }
        }
        Ok(())
    }
}
