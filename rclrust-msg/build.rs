use std::{env, path::PathBuf};

use anyhow::{anyhow, Context as _, Result};
use rclrust_msg_gen::CompileConfig;

fn main() -> Result<()> {
    println!("cargo:rerun-if-env-changed=AMENT_PREFIX_PATH");
    let ament_prefix_paths = env::var("AMENT_PREFIX_PATH")
        .with_context(|| anyhow!("$AMENT_PREFIX_PATH is supposed to be set."))?;
    for ament_prefix_path in ament_prefix_paths.split(':') {
        println!("cargo:rustc-link-search=native={}/lib", ament_prefix_path);
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_file = out_dir.join("idl.rs");

    CompileConfig::new()
        .include_default_rosidl(true)
        .out_file(&out_file)
        .compile()?;

    Ok(())
}
