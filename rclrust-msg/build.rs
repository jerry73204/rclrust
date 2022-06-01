use anyhow::Result;
use rclrust_msg_gen::CompileConfig;

fn main() -> Result<()> {
    CompileConfig::new().run()?;
    Ok(())
}
