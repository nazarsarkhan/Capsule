use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::Regex;
use walkdir::WalkDir;

use crate::resolver::node_resolver::package_root;

const BUILTINS: &[&str] = &[
    "fs", "path", "http", "https", "crypto", "url", "os", "util", "events", "stream",
];

pub fn scan_path(path: &Path) -> Result<Vec<String>> {
    let mut packages = BTreeSet::new();
    let import_from_re = Regex::new(r#"(?m)\bfrom\s*["']([^"']+)["']"#).unwrap();
    let bare_import_re = Regex::new(r#"(?m)^\s*import\s*["']([^"']+)["']"#).unwrap();
    let require_re = Regex::new(r#"require\(\s*["']([^"']+)["']\s*\)"#).unwrap();
    for file in js_files(path) {
        let source = fs::read_to_string(file)?;
        for caps in import_from_re
            .captures_iter(&source)
            .chain(bare_import_re.captures_iter(&source))
            .chain(require_re.captures_iter(&source))
        {
            let spec = caps.get(1).unwrap().as_str();
            if spec.starts_with("./") || spec.starts_with("../") || spec.starts_with('/') {
                continue;
            }
            let pkg = package_root(spec);
            if !BUILTINS.contains(&pkg.as_str()) && !pkg.starts_with("node:") {
                packages.insert(pkg);
            }
        }
    }
    Ok(packages.into_iter().collect())
}

fn js_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("js" | "mjs" | "cjs" | "ts" | "tsx")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_node_imports_and_filters_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("index.js");
        fs::write(&file, r#"import express from "express"; const fs = require("fs"); const x = require("@a/b/c");"#).unwrap();
        assert_eq!(scan_path(&file).unwrap(), vec!["@a/b", "express"]);
    }

    #[test]
    fn scans_typescript_named_imports() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.ts");
        fs::write(
            &file,
            r#"import { z } from "zod"; console.log(z.string().parse("ok"))"#,
        )
        .unwrap();
        assert_eq!(scan_path(&file).unwrap(), vec!["zod"]);
    }
}
