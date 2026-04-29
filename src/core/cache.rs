use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::core::detector::Language;
use crate::util::paths::capsule_home;

pub fn language_cache(language: Language) -> Result<PathBuf> {
    let path = language_cache_path(language)?;
    std::fs::create_dir_all(&path).with_context(|| format!("create cache {}", path.display()))?;
    Ok(path)
}

pub fn language_cache_path(language: Language) -> Result<PathBuf> {
    let base = capsule_home()?;
    let path = match language {
        Language::Python => base.join("python").join("envs"),
        Language::Node => base.join("node").join("envs"),
        Language::TypeScript => base.join("typescript").join("envs"),
    };
    Ok(path)
}

pub fn env_path(language: Language, hash: &str) -> Result<PathBuf> {
    Ok(language_cache(language)?.join(hash))
}

pub fn env_path_no_create(language: Language, hash: &str) -> Result<PathBuf> {
    Ok(language_cache_path(language)?.join(hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_contains_language() {
        let p = language_cache(Language::Python).unwrap();
        assert!(p.to_string_lossy().contains("python"));
    }

    #[test]
    fn no_create_cache_path_does_not_create_env() {
        let hash = format!("test-no-create-{}", std::process::id());
        let path = env_path_no_create(Language::Node, &hash).unwrap();
        if path.exists() {
            std::fs::remove_dir_all(&path).unwrap();
        }

        let computed = env_path_no_create(Language::Node, &hash).unwrap();

        assert_eq!(computed, path);
        assert!(!path.exists());
    }
}
