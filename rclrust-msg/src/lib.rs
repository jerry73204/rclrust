#![warn(rust_2018_idioms, elided_lifetimes_in_paths)]
// #![allow(clippy::all)]

pub mod _core;

include!(concat!(env!("OUT_DIR"), "/rust_src/ffi.rs"));
