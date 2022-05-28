#![warn(
    rust_2018_idioms,
    elided_lifetimes_in_paths,
    clippy::all,
    clippy::nursery
)]

mod compile;
mod parser;
mod types;

pub use crate::{compile::*, parser::package::*};
