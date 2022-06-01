use anyhow::Result;
use rclrust_msg_gen::CompileConfig;

fn main() -> Result<()> {
    CompileConfig::new_ros2().run()?;
    Ok(())
}
