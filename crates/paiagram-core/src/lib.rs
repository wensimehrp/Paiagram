//! The core of the Paiagram application. This crate contains the systems used in the runtime and
//! the types.

pub mod colors;
pub mod entry;
pub mod export;
pub mod graph;
pub mod i18n;
pub mod import;
pub mod interval;
pub mod plugin;
pub mod problems;
pub mod route;
pub mod settings;
pub mod station;
pub mod trip;
pub mod units;
pub mod vehicle;

pub use trip::class;
