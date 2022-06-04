use anyhow::Result;
use rclrust_msg_gen::CompileConfig;

fn main() -> Result<()> {
    let mut compiler = CompileConfig::new_ros2()
        .exclude_package("example_interfaces")
        .build()?;
    compiler.codegen()?;
    compiler.static_link()?;
    Ok(())
}
