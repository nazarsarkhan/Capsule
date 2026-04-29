use std::fs;
use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::core::detector::Language;
use crate::core::lockfile::{lock_path, CapsuleLock};

pub fn project_hash(
    root: &Path,
    language: Language,
    runtime_version: &str,
    lock: Option<&CapsuleLock>,
) -> Result<String> {
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let lock_content = if let Some(lock) = lock {
        serde_json::to_string(lock)?
    } else {
        let path = lock_path(root);
        if path.exists() {
            fs::read_to_string(path)?
        } else {
            String::new()
        }
    };
    let input = format!(
        "{}\n{}\n{}\n{}",
        canonical.display(),
        language.as_str(),
        runtime_version,
        lock_content
    );
    let hash = Sha256::digest(input.as_bytes());
    Ok(format!("{hash:x}")[..16].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let a = project_hash(dir.path(), Language::Python, "3.11", None).unwrap();
        let b = project_hash(dir.path(), Language::Python, "3.11", None).unwrap();
        assert_eq!(a, b);
    }
}
