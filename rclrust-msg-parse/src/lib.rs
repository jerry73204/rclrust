// #![warn(
//     rust_2018_idioms,
//     elided_lifetimes_in_paths,
//     clippy::all,
//     clippy::nursery
// )]

mod parser;
mod types;

use std::{fs, path::Path};

use anyhow::Result;
use quote::quote;

pub use crate::parser::get_packages;

pub fn compile_packages(
    input_dirs: &[impl AsRef<Path>],
    output_dir: impl AsRef<Path>,
) -> Result<()> {
    let output_dir = output_dir.as_ref();

    get_packages(input_dirs)?
        .iter()
        .try_for_each(|pkg| -> Result<()> {
            let output_file = output_dir.join(format!("{}.rs", pkg.name));
            let token_stream = pkg.token_stream();
            let content = (quote! { #token_stream }).to_string();
            fs::write(output_file, &content)?;
            Ok(())
        })?;
    Ok(())
}
