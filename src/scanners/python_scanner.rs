use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

const STDLIB: &[&str] = &[
    "abc",
    "argparse",
    "asyncio",
    "collections",
    "contextlib",
    "csv",
    "dataclasses",
    "datetime",
    "decimal",
    "functools",
    "hashlib",
    "http",
    "importlib",
    "inspect",
    "io",
    "itertools",
    "json",
    "logging",
    "math",
    "os",
    "pathlib",
    "random",
    "re",
    "shutil",
    "sqlite3",
    "statistics",
    "string",
    "subprocess",
    "sys",
    "tempfile",
    "threading",
    "time",
    "typing",
    "unittest",
    "urllib",
    "uuid",
    "venv",
    "xml",
];

pub fn scan_path(path: &Path, root: &Path) -> Result<Vec<String>> {
    let mut imports = BTreeSet::new();
    for file in python_files(path) {
        scan_file(&file, root, &mut imports)?;
    }
    Ok(imports.into_iter().collect())
}

fn python_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("py"))
        .collect()
}

fn scan_file(file: &Path, root: &Path, imports: &mut BTreeSet<String>) -> Result<()> {
    let source = fs::read_to_string(file)?;
    scan_source(&source, root, imports);
    Ok(())
}

fn scan_source(source: &str, root: &Path, imports: &mut BTreeSet<String>) {
    for line in source.lines() {
        let code = line
            .trim_start_matches('\u{feff}')
            .split('#')
            .next()
            .unwrap_or_default()
            .trim();
        if code.is_empty() || code.starts_with("from .") || code.starts_with("from\t.") {
            continue;
        }
        if let Some(rest) = code.strip_prefix("import ") {
            let import_clause = rest.split(';').next().unwrap_or_default();
            for part in import_clause.split(',') {
                let module = part.split_whitespace().next().unwrap_or_default();
                add_if_external(module, root, imports);
            }
            continue;
        }
        if let Some(rest) = code.strip_prefix("from ") {
            let import_clause = rest.split(';').next().unwrap_or_default();
            if let Some((module, _)) = import_clause.split_once(" import ") {
                add_if_external(module.trim(), root, imports);
            }
        }
    }
}

fn add_if_external(module: &str, root: &Path, imports: &mut BTreeSet<String>) {
    if module.is_empty() || module.starts_with('.') {
        return;
    }
    let name = module.split('.').next().unwrap_or_default();
    if is_identifier(name) && !is_stdlib(name) && !is_local(name, root) {
        imports.insert(name.to_string());
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

pub fn is_stdlib(name: &str) -> bool {
    STDLIB.contains(&name)
}

fn is_local(name: &str, root: &Path) -> bool {
    root.join(format!("{name}.py")).exists() || root.join(name).join("__init__.py").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_python_imports_and_filters() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("helpers.py"), "").unwrap();
        let file = dir.path().join("app.py");
        fs::write(
            &file,
            "import os\nimport requests.sessions, pandas as pd\nfrom fastapi import FastAPI\nimport helpers\nfrom .local import thing\n",
        )
        .unwrap();
        assert_eq!(
            scan_path(&file, dir.path()).unwrap(),
            vec!["fastapi", "pandas", "requests"]
        );
    }

    #[test]
    fn scans_inline_semicolon_import() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.py");
        fs::write(&file, "import requests; print(requests.__version__)\n").unwrap();

        assert_eq!(scan_path(&file, dir.path()).unwrap(), vec!["requests"]);
    }

    #[test]
    fn scans_inline_semicolon_import_with_utf8_bom() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.py");
        fs::write(
            &file,
            "\u{feff}import requests; print(requests.__version__)\n",
        )
        .unwrap();

        assert_eq!(scan_path(&file, dir.path()).unwrap(), vec!["requests"]);
    }
}
