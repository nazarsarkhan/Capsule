use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::detector::Language;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runtime {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapsuleLock {
    pub version: u8,
    pub project_hash: String,
    pub language: Language,
    pub runtime: Runtime,
    pub packages: BTreeMap<String, Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Package {
    pub version: String,
}

pub fn lock_path(root: &Path) -> PathBuf {
    root.join("capsule.lock")
}

pub fn read(root: &Path) -> Result<Option<CapsuleLock>> {
    let path = lock_path(root);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
}

pub fn write(root: &Path, lock: &CapsuleLock) -> Result<()> {
    let path = lock_path(root);
    let contents = serde_json::to_string_pretty(lock)? + "\n";
    if path.exists() && fs::read_to_string(&path)? == contents {
        return Ok(());
    }
    fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut packages = BTreeMap::new();
        packages.insert(
            "requests".into(),
            Package {
                version: "2.32.3".into(),
            },
        );
        let lock = CapsuleLock {
            version: 1,
            project_hash: "abc".into(),
            language: Language::Python,
            runtime: Runtime {
                name: "python".into(),
                version: "3.11.8".into(),
            },
            packages,
        };
        write(dir.path(), &lock).unwrap();
        assert_eq!(read(dir.path()).unwrap(), Some(lock));
    }
}
