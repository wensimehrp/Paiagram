pub use ast::SerializeToOud;
pub use ast::Structure;
pub use ir::*;
pub use time::Time;
pub use timetable::{ServiceMode, TimetableEntry};

use crate::ast::parse_to_ast;

pub mod ast;
pub mod operation;
pub mod ir;
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
        let mut items = smallvec::SmallVec::new();
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

pub fn parse_to_ir(input: &str) -> Result<Root, IrConversionError> {
    let v = parse_to_ast(input).map_err(IrConversionError::from)?;
    Root::try_from(v.as_slice())
}
