use anyhow::Result;

use crate::util::paths::capsule_home;

pub fn clean(python: bool, node: bool, all: bool) -> Result<()> {
    let home = capsule_home()?;
    let mut targets = Vec::new();
    if all || (!python && !node) {
        targets.push(home.clone());
    } else {
        if python {
            targets.push(home.join("python"));
        }
        if node {
            targets.push(home.join("node"));
            targets.push(home.join("typescript"));
        }
    }
    for target in targets {
        if target.exists() {
            println!("Capsule: removing {}", target.display());
            std::fs::remove_dir_all(&target)?;
        }
    }
    Ok(())
}
