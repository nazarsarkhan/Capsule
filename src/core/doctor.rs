use anyhow::Result;

use crate::core::command::command_exists;

pub fn doctor() -> Result<()> {
    println!("Capsule Doctor");
    println!("---------------");
    println!("Python:");
    for tool in ["py", "python3", "python", "pip", "uv"] {
        let status = if command_exists(tool) {
            "found"
        } else {
            "missing"
        };
        println!("  {tool}: {status}");
    }
    println!();
    println!("Node:");
    for tool in ["node", "npm", "npx"] {
        let status = if command_exists(tool) {
            "found"
        } else {
            "missing"
        };
        println!("  {tool}: {status}");
    }
    Ok(())
}
