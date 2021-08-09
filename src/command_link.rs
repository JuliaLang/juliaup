use anyhow::{Result};

pub fn run_command_link(channel: String, file: String, args: Vec<String>) -> Result<()> {
    println!("{}, {}, {}", channel, file, args.len());
    Ok(())
}
