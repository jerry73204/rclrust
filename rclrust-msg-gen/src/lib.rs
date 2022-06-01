#![warn(
    rust_2018_idioms,
    elided_lifetimes_in_paths,
    clippy::all,
    clippy::nursery
)]

pub mod compiler;
pub mod config;

pub use compiler::*;
pub use config::*;
