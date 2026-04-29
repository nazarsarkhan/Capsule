use std::path::PathBuf;

use anyhow::{anyhow, Result};

pub fn capsule_home() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not locate home directory"))?;
    Ok(home.join(".capsule"))
}
