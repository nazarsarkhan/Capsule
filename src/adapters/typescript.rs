use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::adapters::node::{
    create_dir_link, ensure_node_env, install_node_packages, linked_project_root, node_runtime,
    npm_list_versions,
};
use crate::adapters::Adapter;
use crate::core::cache;
use crate::core::command::resolve_command;
use crate::core::detector::Project;
use crate::core::lockfile::{self, CapsuleLock};
use crate::core::runner::fmt_packages;
use crate::scanners::node_scanner;
use crate::security::trust;

pub struct TypeScriptAdapter;

impl Adapter for TypeScriptAdapter {
    fn scan(&self, project: &Project) -> Result<Vec<String>> {
        let mut packages = node_scanner::scan_path(&project.input)?;
        if !packages.iter().any(|p| p == "tsx") {
            packages.push("tsx".into());
        }
        Ok(packages)
    }

    fn lock(&self, project: &Project, yes: bool, no_install: bool, _verbose: bool) -> Result<()> {
        let packages = self.scan(project)?;
        if no_install {
            trust::report_unknown_no_install(&packages)?;
            println!(
                "Capsule: no-install mode: would lock {}",
                fmt_packages(&packages)
            );
            return Ok(());
        }
        trust::confirm_unknown(&packages, yes)?;
        let env = ensure_node_env(project)?;
        install_node_packages(&env, &packages)?;
        let lock = crate::core::runner::base_lock(
            project,
            node_runtime(),
            npm_list_versions(&env, &packages)?,
        )?;
        println!("Capsule: writing capsule.lock");
        lockfile::write(&project.root, &lock)
    }

    fn run(&self, project: &Project, yes: bool, no_install: bool, _verbose: bool) -> Result<()> {
        println!("Capsule: scanning imports...");
        let packages = self.scan(project)?;
        println!("Capsule: packages: {}", fmt_packages(&packages));
        let existing = lockfile::read(&project.root)?;
        let runtime = node_runtime();
        let hash = crate::util::hashing::project_hash(
            &project.root,
            project.language,
            &runtime.version,
            existing.as_ref(),
        )?;
        let env = cache::env_path(project.language, &hash)?;
        println!("Capsule: using cache {}", env.display());
        if no_install {
            trust::report_unknown_no_install(&packages)?;
            println!(
                "Capsule: no-install mode: would install {}",
                fmt_packages(&packages)
            );
            return Ok(());
        }
        trust::confirm_unknown(&packages, yes)?;
        crate::adapters::node::init_node_env(&env)?;
        let install_targets = typescript_install_targets(&packages, existing.as_ref());
        install_node_packages(&env, &install_targets)?;
        if lock_needs_update(existing.as_ref(), &packages) {
            let lock = CapsuleLock {
                version: 1,
                project_hash: hash,
                language: project.language,
                runtime,
                packages: npm_list_versions(&env, &packages)?,
            };
            println!("Capsule: writing capsule.lock");
            lockfile::write(&project.root, &lock)?;
        }
        run_ts(project, &env)
    }
}

fn run_ts(project: &Project, env: &Path) -> Result<()> {
    if project.is_file {
        let original_entry = project
            .entry
            .as_ref()
            .context("TypeScript file entry missing")?;
        println!("Capsule: preparing TypeScript workspace...");
        let entry = prepare_typescript_workspace(original_entry, env)?;
        let bin = if cfg!(windows) {
            env.join("node_modules").join(".bin").join("tsx.cmd")
        } else {
            env.join("node_modules").join(".bin").join("tsx")
        };
        println!("Capsule: running {}", entry.display());
        return run_checked(
            Command::new(bin)
                .arg(&entry)
                .current_dir(env.join("workspace")),
        );
    }
    let package_json = project.root.join("package.json");
    if package_json.exists() {
        let json: Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
        let scripts = json.get("scripts").and_then(Value::as_object);
        let linked_root = linked_project_root(project, env)?;
        if scripts.and_then(|s| s.get("dev")).is_some() {
            println!("Capsule: running npm run dev");
            let npm = resolve_command("npm").context("npm was not found on PATH")?;
            return run_checked(
                Command::new(npm)
                    .args(["run", "dev"])
                    .current_dir(linked_root),
            );
        }
        if scripts.and_then(|s| s.get("start")).is_some() {
            println!("Capsule: running npm start");
            let npm = resolve_command("npm").context("npm was not found on PATH")?;
            return run_checked(Command::new(npm).arg("start").current_dir(linked_root));
        }
    }
    bail!("TypeScript directory run needs a dev/start script for this MVP")
}

fn prepare_typescript_workspace(original_file: &Path, env: &Path) -> Result<PathBuf> {
    let workspace = env.join("workspace");
    fs::create_dir_all(&workspace)?;
    ensure_workspace_node_modules(&workspace, env)?;

    let file_name = original_file
        .file_name()
        .context("TypeScript file path has no filename")?;
    let workspace_file = workspace.join(file_name);
    fs::copy(original_file, &workspace_file).with_context(|| {
        format!(
            "copy TypeScript entry {} to {}",
            original_file.display(),
            workspace_file.display()
        )
    })?;
    Ok(workspace_file)
}

fn ensure_workspace_node_modules(workspace: &Path, env: &Path) -> Result<()> {
    let workspace_node_modules = workspace.join("node_modules");
    let env_node_modules = env.join("node_modules");
    fs::create_dir_all(&env_node_modules)?;
    if workspace_node_modules.exists() {
        return Ok(());
    }
    create_dir_link(&env_node_modules, &workspace_node_modules)
}

fn typescript_install_targets(packages: &[String], existing: Option<&CapsuleLock>) -> Vec<String> {
    packages
        .iter()
        .map(|package| {
            existing
                .and_then(|lock| lock.packages.get(package))
                .map(|pinned| format!("{package}@{}", pinned.version))
                .unwrap_or_else(|| package.clone())
        })
        .collect()
}

fn lock_needs_update(existing: Option<&CapsuleLock>, packages: &[String]) -> bool {
    existing
        .map(|lock| {
            packages
                .iter()
                .any(|package| !lock.packages.contains_key(package))
        })
        .unwrap_or(true)
}

fn run_checked(cmd: &mut Command) -> Result<()> {
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.status()?
        .success()
        .then_some(())
        .context("command failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lockfile::Package;

    #[test]
    fn prepares_typescript_workspace_file_path() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("app.ts");
        fs::write(&source, r#"console.log("ok")"#).unwrap();
        let env = temp.path().join("env");
        let project = temp.path().join("project");
        fs::create_dir(&project).unwrap();

        let workspace_file = prepare_typescript_workspace(&source, &env).unwrap();

        assert_eq!(workspace_file, env.join("workspace").join("app.ts"));
        assert_eq!(
            fs::read_to_string(workspace_file).unwrap(),
            r#"console.log("ok")"#
        );
        assert!(!project.join("node_modules").exists());
        assert!(!project.join(".venv").exists());
        assert!(env.join("workspace").join("node_modules").exists());
    }

    #[test]
    fn builds_typescript_install_list_with_tsx() {
        let discovered = vec!["zod".to_string(), "tsx".to_string()];
        assert_eq!(typescript_install_targets(&discovered, None), discovered);
    }

    #[test]
    fn keeps_discovered_packages_when_lock_is_missing_one() {
        let mut packages = std::collections::BTreeMap::new();
        packages.insert(
            "tsx".to_string(),
            Package {
                version: "4.21.0".to_string(),
            },
        );
        let lock = CapsuleLock {
            version: 1,
            project_hash: "hash".to_string(),
            language: crate::core::detector::Language::TypeScript,
            runtime: crate::core::lockfile::Runtime {
                name: "node".to_string(),
                version: "20.0.0".to_string(),
            },
            packages,
        };

        assert_eq!(
            typescript_install_targets(&["zod".to_string(), "tsx".to_string()], Some(&lock)),
            vec!["zod".to_string(), "tsx@4.21.0".to_string()]
        );
        assert!(lock_needs_update(
            Some(&lock),
            &["zod".to_string(), "tsx".to_string()]
        ));
    }
}
