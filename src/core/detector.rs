use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    Node,
    TypeScript,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Node => "node",
            Self::TypeScript => "typescript",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub input: PathBuf,
    pub root: PathBuf,
    pub language: Language,
    pub entry: Option<PathBuf>,
    pub is_file: bool,
}

pub fn detect(path: &Path) -> Result<Project> {
    let input = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if input.is_file() {
        let language = detect_file(&input)?;
        let root = input.parent().unwrap_or(Path::new(".")).to_path_buf();
        return Ok(Project {
            input: input.clone(),
            root,
            language,
            entry: Some(input),
            is_file: true,
        });
    }
    if input.is_dir() {
        let language = detect_dir(&input)?;
        return Ok(Project {
            input,
            root: path.to_path_buf().canonicalize()?,
            language,
            entry: None,
            is_file: false,
        });
    }
    bail!("path does not exist: {}", path.display())
}

fn detect_file(path: &Path) -> Result<Language> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
    {
        "py" => Ok(Language::Python),
        "js" | "mjs" | "cjs" => Ok(Language::Node),
        "ts" | "tsx" => Ok(Language::TypeScript),
        ext => bail!("unsupported file extension: {ext}"),
    }
}

fn detect_dir(dir: &Path) -> Result<Language> {
    if dir.join("package.json").exists() && dir.join("tsconfig.json").exists() {
        return Ok(Language::TypeScript);
    }
    if dir.join("package.json").exists() {
        return Ok(Language::Node);
    }
    if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
        return Ok(Language::Python);
    }
    bail!("could not detect project language in {}", dir.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_file_extension() {
        assert_eq!(detect_file(Path::new("app.py")).unwrap(), Language::Python);
        assert_eq!(detect_file(Path::new("index.js")).unwrap(), Language::Node);
        assert_eq!(
            detect_file(Path::new("app.ts")).unwrap(),
            Language::TypeScript
        );
    }

    #[test]
    fn directory_priority_prefers_typescript_package() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        assert_eq!(detect_dir(dir.path()).unwrap(), Language::TypeScript);
    }
}
