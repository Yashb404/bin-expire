use crate::models::BinaryInfo;
use anyhow::Result;

pub fn archive_binary(bin: &BinaryInfo, archive_dir: &std::path::Path) -> Result<()> {
    println!("Archiving: {}", bin.name);
    // TODO: Implement file moving logic
    Ok(())
}

pub fn restore_binary(name: &str) -> Result<()> {
    println!("Restoring: {}", name);
    // TODO: Implement restore logic
    Ok(())
}