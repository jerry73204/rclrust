use anyhow::Result;
use rclrust_msg_gen::CompileConfig;

fn main() -> Result<()> {
    let build = cc::Build::new();
    let output = CompileConfig::new().compile_ffi(build).run()?;
    output.build_commands.iter().for_each(|cmd| {
        println!("{}", cmd);
    });
    output.cc_build.unwrap().compile(env!("CARGO_PKG_NAME"));
    Ok(())
}
