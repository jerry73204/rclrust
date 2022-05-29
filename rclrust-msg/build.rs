use anyhow::Result;
use rclrust_msg_gen::CompileConfig;

fn main() -> Result<()> {
    let output = CompileConfig::new().run()?;
    output.build_commands.iter().for_each(|cmd| {
        println!("{}", cmd);
    });
    Ok(())
}
