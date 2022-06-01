#![warn(
    rust_2018_idioms,
    elided_lifetimes_in_paths,
    clippy::all,
    clippy::nursery
)]

pub mod parser;
pub mod types;

pub use crate::parser::package::*;
