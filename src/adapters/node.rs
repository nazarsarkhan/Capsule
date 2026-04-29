use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::Value;

use crate::adapters::Adapter;
use crate::core::cache;
use crate::core::command::{command_output, resolve_command};
use crate::core::detector::Project;
use crate::core::lockfile::{self, CapsuleLock, Package, Runtime};
use crate::core::runner::{base_lock, fmt_packages};
use crate::scanners::node_scanner;
use crate::security::trust;

pub struct NodeAdapter;

impl Adapter for NodeAdapter {
    fn scan(&self, project: &Project) -> Result<Vec<String>> {
        node_scanner::scan_path(&project.input)
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
        let lock = base_lock(project, node_runtime(), npm_list_versions(&env, &packages)?)?;
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
        init_node_env(&env)?;
        let install_targets = node_install_targets(&packages, existing.as_ref());
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
        run_node_entry(project, &env, yes)
    }
}

pub fn ensure_node_env(project: &Project) -> Result<PathBuf> {
    let runtime = node_runtime();
    let existing = lockfile::read(&project.root)?;
    let hash = crate::util::hashing::project_hash(
        &project.root,
        project.language,
        &runtime.version,
        existing.as_ref(),
    )?;
    let env = cache::env_path(project.language, &hash)?;
    println!("Capsule: using cache {}", env.display());
    init_node_env(&env)?;
    Ok(env)
}

pub fn init_node_env(env: &Path) -> Result<()> {
    fs::create_dir_all(env)?;
    let pkg = env.join("package.json");
    if !pkg.exists() {
        fs::write(pkg, r#"{"name":"capsule-env","private":true}"#)?;
    }
    Ok(())
}

pub fn install_node_packages(env: &Path, packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }
    let npm = resolve_command("npm").context("npm was not found on PATH")?;
    println!("Capsule: installing npm packages: {}", packages.join(", "));
    run_checked(
        Command::new(npm)
            .arg("install")
            .arg("--silent")
            .args(packages)
            .current_dir(env),
    )
}

pub fn run_node_entry(project: &Project, env: &Path, yes: bool) -> Result<()> {
    if !project.is_file {
        if let Some(script) = preferred_package_script(&project.root)? {
            let linked_root = linked_project_root(project, env)?;
            println!("Capsule: running npm run {script}");
            let npm = resolve_command("npm").context("npm was not found on PATH")?;
            return run_checked(
                Command::new(npm)
                    .args(["run", &script])
                    .current_dir(linked_root),
            );
        }
    }
    let linked_entry = linked_project_entry(project, env)?;
    println!("Capsule: running {}", linked_entry.display());
    let result = run_node(project, &linked_entry)?;
    if result.success() {
        return Ok(());
    }
    if let Some(pkg) = parse_module_not_found(&result.stderr) {
        println!("Capsule: runtime missing package: {pkg}");
        trust::confirm_unknown(std::slice::from_ref(&pkg), yes)?;
        install_node_packages(env, std::slice::from_ref(&pkg))?;
        update_node_lock(env, project, &[pkg])?;
        let retry = run_node(project, &linked_entry)?;
        if !retry.success() {
            bail!("node script failed after retry");
        }
    } else {
        bail!("node script failed");
    }
    Ok(())
}

pub fn linked_project_root(project: &Project, env: &Path) -> Result<PathBuf> {
    let link = env.join("project");
    ensure_project_link(&project.root, &link, env)?;
    Ok(link)
}

struct RunResult {
    code: i32,
    stderr: String,
}

impl RunResult {
    fn success(&self) -> bool {
        self.code == 0
    }
}

fn run_node(project: &Project, entry: &Path) -> Result<RunResult> {
    let output = Command::new("node")
        .arg("--preserve-symlinks")
        .arg("--preserve-symlinks-main")
        .arg(entry)
        .current_dir(&project.root)
        .output()?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(RunResult {
        code: output.status.code().unwrap_or(1),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

pub fn linked_project_entry(project: &Project, env: &Path) -> Result<PathBuf> {
    let link = env.join("project");
    ensure_project_link(&project.root, &link, env)?;
    let entry = if let Some(entry) = &project.entry {
        entry.clone()
    } else {
        detect_node_entry(&project.root)?
    };
    let rel = entry.strip_prefix(&project.root).with_context(|| {
        format!(
            "entry {} is not inside project root {}",
            entry.display(),
            project.root.display()
        )
    })?;
    Ok(link.join(rel))
}

fn detect_node_entry(root: &Path) -> Result<PathBuf> {
    let package_json = root.join("package.json");
    if package_json.exists() {
        let value: Value = serde_json::from_str(&fs::read_to_string(&package_json)?)?;
        if let Some(main) = value.get("main").and_then(Value::as_str) {
            let candidate = root.join(main);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    for name in ["index.js", "server.js", "app.js", "main.js"] {
        let candidate = root.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("Node directory run needs package.json main or index.js/server.js/app.js/main.js")
}

fn preferred_package_script(root: &Path) -> Result<Option<String>> {
    let package_json = root.join("package.json");
    if !package_json.exists() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
    let scripts = value.get("scripts").and_then(Value::as_object);
    if scripts.and_then(|s| s.get("dev")).is_some() {
        return Ok(Some("dev".into()));
    }
    if scripts.and_then(|s| s.get("start")).is_some() {
        return Ok(Some("start".into()));
    }
    Ok(None)
}

fn ensure_project_link(target: &Path, link: &Path, env: &Path) -> Result<()> {
    if !link.exists() {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }
        create_dir_link(target, link).with_context(|| {
            format!(
                "create project link {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }
    ensure_project_node_modules(link, env)
        .with_context(|| format!("create project node_modules link under {}", link.display()))
}

fn ensure_project_node_modules(project_link: &Path, env: &Path) -> Result<()> {
    let project_node_modules = project_node_modules_path(project_link);
    let env_node_modules = env.join("node_modules");
    fs::create_dir_all(&env_node_modules)?;

    if project_node_modules.exists() {
        if path_points_to(&project_node_modules, &env_node_modules) {
            return Ok(());
        }
        if is_link_like(&project_node_modules) {
            remove_link_like(&project_node_modules)?;
        } else {
            println!(
                "Capsule: preserving existing project node_modules at {}",
                project_node_modules.display()
            );
            return Ok(());
        }
    }

    create_dir_link(&env_node_modules, &project_node_modules)
}

fn project_node_modules_path(project_link: &Path) -> PathBuf {
    project_link.join("node_modules")
}

fn path_points_to(path: &Path, target: &Path) -> bool {
    match (fs::canonicalize(path), fs::canonicalize(target)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(not(windows))]
fn is_link_like(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_link_like(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    fs::symlink_metadata(path)
        .map(|metadata| {
            metadata.file_type().is_symlink()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        })
        .unwrap_or(false)
}

fn remove_link_like(path: &Path) -> Result<()> {
    fs::remove_dir(path)?;
    Ok(())
}

#[cfg(unix)]
pub fn create_dir_link(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
pub fn create_dir_link(target: &Path, link: &Path) -> Result<()> {
    match std::os::windows::fs::symlink_dir(target, link) {
        Ok(()) => Ok(()),
        Err(_) => {
            let status = Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .status()?;
            status.success().then_some(()).context("mklink /J failed")
        }
    }
}

pub fn npm_list_versions(env: &Path, packages: &[String]) -> Result<BTreeMap<String, Package>> {
    let npm = resolve_command("npm").context("npm was not found on PATH")?;
    let output = Command::new(npm)
        .args(["list", "--json", "--depth=0"])
        .current_dir(env)
        .output()?;
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    let deps = value.get("dependencies").and_then(Value::as_object);
    let mut out = BTreeMap::new();
    if let Some(deps) = deps {
        for package in packages {
            if let Some(version) = deps
                .get(package)
                .and_then(|v| v.get("version"))
                .and_then(Value::as_str)
            {
                out.insert(
                    package.clone(),
                    Package {
                        version: version.to_string(),
                    },
                );
            }
        }
    }
    let missing: Vec<_> = packages
        .iter()
        .filter(|package| !out.contains_key(*package))
        .cloned()
        .collect();
    if !missing.is_empty() {
        bail!(
            "could not resolve exact npm version(s): {}",
            missing.join(", ")
        );
    }
    Ok(out)
}

fn update_node_lock(env: &Path, project: &Project, packages: &[String]) -> Result<()> {
    let mut lock = lockfile::read(&project.root)?.unwrap_or_else(|| CapsuleLock {
        version: 1,
        project_hash: String::new(),
        language: project.language,
        runtime: node_runtime(),
        packages: BTreeMap::new(),
    });
    lock.packages.extend(npm_list_versions(env, packages)?);
    lock.project_hash = crate::util::hashing::project_hash(
        &project.root,
        project.language,
        &lock.runtime.version,
        Some(&lock),
    )?;
    lockfile::write(&project.root, &lock)
}

fn node_install_targets(packages: &[String], existing: Option<&CapsuleLock>) -> Vec<String> {
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
            let same_len = lock.packages.len() == packages.len();
            let has_all_current = packages
                .iter()
                .all(|package| lock.packages.contains_key(package));
            !(same_len && has_all_current)
        })
        .unwrap_or(true)
}

pub fn node_runtime() -> Runtime {
    Runtime {
        name: "node".into(),
        version: command_output("node", &["--version"])
            .unwrap_or_else(|| "unknown".into())
            .trim_start_matches('v')
            .to_string(),
    }
}

fn parse_module_not_found(stderr: &str) -> Option<String> {
    let patterns = [
        r#"Cannot find module ['"]([^'"]+)['"]"#,
        r#"Cannot find package ['"]([^'"]+)['"]"#,
    ];
    let spec = patterns.iter().find_map(|pattern| {
        Regex::new(pattern)
            .ok()?
            .captures(stderr)?
            .get(1)
            .map(|m| m.as_str().to_string())
    })?;
    if spec.starts_with("./") || spec.starts_with("../") {
        None
    } else {
        Some(crate::resolver::node_resolver::package_root(&spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_missing_module() {
        assert_eq!(
            parse_module_not_found("Error: Cannot find module '@scope/pkg/sub'"),
            Some("@scope/pkg".into())
        );
    }

    #[test]
    fn computes_project_node_modules_path() {
        let project_link = Path::new("C:/Users/example/.capsule/node/envs/hash/project");
        assert_eq!(
            project_node_modules_path(project_link),
            project_link.join("node_modules")
        );
    }

    #[test]
    fn node_install_targets_ignore_stale_lock_packages() {
        let mut locked = BTreeMap::new();
        locked.insert(
            "express".to_string(),
            Package {
                version: "5.2.1".to_string(),
            },
        );
        locked.insert(
            "body-parser".to_string(),
            Package {
                version: "2.2.2".to_string(),
            },
        );
        let lock = CapsuleLock {
            version: 1,
            project_hash: "hash".to_string(),
            language: crate::core::detector::Language::Node,
            runtime: Runtime {
                name: "node".to_string(),
                version: "22.0.0".to_string(),
            },
            packages: locked,
        };

        assert_eq!(
            node_install_targets(&["express".to_string()], Some(&lock)),
            vec!["express@5.2.1".to_string()]
        );
        assert!(lock_needs_update(Some(&lock), &["express".to_string()]));
    }
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
