use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use regex::Regex;

use crate::adapters::Adapter;
use crate::core::cache;
use crate::core::command::command_exists;
use crate::core::detector::Project;
use crate::core::lockfile::{self, CapsuleLock, Package, Runtime};
use crate::core::runner::{base_lock, fmt_packages};
use crate::resolver::python_resolver;
use crate::scanners::python_scanner;
use crate::security::trust;

pub struct PythonAdapter;

impl Adapter for PythonAdapter {
    fn scan(&self, project: &Project) -> Result<Vec<String>> {
        let imports = python_scanner::scan_path(&project.input, &project.root)?;
        Ok(imports
            .into_iter()
            .map(|i| python_resolver::resolve(&i))
            .collect())
    }

    fn lock(&self, project: &Project, yes: bool, no_install: bool, _verbose: bool) -> Result<()> {
        let packages = self.scan(project)?;
        if no_install {
            report_unknown_python(project)?;
            println!(
                "Capsule: no-install mode: would lock {}",
                fmt_packages(&packages)
            );
            return Ok(());
        }
        guard_unknown_python(project, yes)?;
        let env = ensure_python_env(project)?;
        install_python_packages(project, &env, &packages)?;
        let versions = freeze_selected(&env, &packages)?;
        let lock = base_lock(project, python_runtime(), versions)?;
        println!("Capsule: writing capsule.lock");
        lockfile::write(&project.root, &lock)
    }

    fn run(&self, project: &Project, yes: bool, no_install: bool, _verbose: bool) -> Result<()> {
        println!("Capsule: scanning imports...");
        let packages = self.scan(project)?;
        println!("Capsule: packages: {}", fmt_packages(&packages));
        let existing = lockfile::read(&project.root)?;
        let install_targets = if let Some(lock) = &existing {
            println!("Capsule: using capsule.lock pinned versions");
            lock.packages
                .iter()
                .map(|(k, v)| format!("{k}=={}", v.version))
                .collect()
        } else {
            packages.clone()
        };
        let runtime = python_runtime();
        let hash = crate::util::hashing::project_hash(
            &project.root,
            project.language,
            &runtime.version,
            existing.as_ref(),
        )?;
        let env = cache::env_path(project.language, &hash)?;
        println!("Capsule: using cache {}", env.display());
        if no_install {
            report_unknown_python(project)?;
            println!(
                "Capsule: no-install mode: would install {}",
                fmt_packages(&install_targets)
            );
            return Ok(());
        }
        guard_unknown_python(project, yes)?;
        create_venv_if_needed(&env)?;
        install_python_packages(project, &env, &install_targets)?;
        if existing.is_none() {
            let versions = freeze_selected(&env, &packages)?;
            let lock = CapsuleLock {
                version: 1,
                project_hash: hash,
                language: project.language,
                runtime,
                packages: versions,
            };
            println!("Capsule: writing capsule.lock");
            lockfile::write(&project.root, &lock)?;
        }
        let entry = project
            .entry
            .as_ref()
            .context("Python run requires a file path")?;
        println!("Capsule: running {}", entry.display());
        let status = run_python(&env, entry)?;
        if status.success() {
            return Ok(());
        }
        if let Some(missing) = parse_module_not_found(&status.stderr) {
            let pkg = python_resolver::resolve(&missing);
            println!("Capsule: script reported missing module: {missing}");
            if python_resolver::is_curated(&missing) {
                trust::validate_packages(std::slice::from_ref(&pkg))?;
            } else {
                trust::confirm_unknown(std::slice::from_ref(&pkg), yes)?;
            }
            install_python_packages(project, &env, std::slice::from_ref(&pkg))?;
            update_lock_with(&env, project, &[pkg])?;
            let retry = run_python(&env, entry)?;
            if !retry.success() {
                bail!("script failed after retry");
            }
        } else {
            bail!("script failed");
        }
        Ok(())
    }
}

fn guard_unknown_python(project: &Project, yes: bool) -> Result<()> {
    let imports = python_scanner::scan_path(&project.input, &project.root)?;
    let unknown: Vec<String> = imports
        .into_iter()
        .filter(|i| !python_resolver::is_curated(i))
        .map(|i| python_resolver::resolve(&i))
        .collect();
    trust::confirm_unknown(&unknown, yes)
}

fn report_unknown_python(project: &Project) -> Result<()> {
    let imports = python_scanner::scan_path(&project.input, &project.root)?;
    let unknown: Vec<String> = imports
        .into_iter()
        .filter(|i| !python_resolver::is_curated(i))
        .map(|i| python_resolver::resolve(&i))
        .collect();
    trust::report_unknown_no_install(&unknown)
}

fn ensure_python_env(project: &Project) -> Result<PathBuf> {
    let runtime = python_runtime();
    let existing = lockfile::read(&project.root)?;
    let hash = crate::util::hashing::project_hash(
        &project.root,
        project.language,
        &runtime.version,
        existing.as_ref(),
    )?;
    let env = cache::env_path(project.language, &hash)?;
    println!("Capsule: using cache {}", env.display());
    create_venv_if_needed(&env)?;
    Ok(env)
}

fn create_venv_if_needed(env: &Path) -> Result<()> {
    if python_bin(env).exists() {
        return Ok(());
    }
    println!("Capsule: creating cached Python environment...");
    let py =
        resolve_python_command().context("could not find Python; tried py -3, python3, python")?;
    println!("Capsule: using Python command: {}", py.display());
    Command::new(&py.program)
        .args(&py.args)
        .args(["-m", "venv"])
        .arg(env)
        .status()?
        .success()
        .then_some(())
        .context("failed to create venv")
}

fn install_python_packages(project: &Project, env: &Path, packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }
    let py = python_bin(env);
    if command_exists("uv") {
        println!("Capsule: installing dependencies with uv...");
        run_checked(
            Command::new("uv")
                .args(["pip", "install", "--python"])
                .arg(&py)
                .arg("-q")
                .args(packages)
                .current_dir(&project.root),
        )
    } else {
        println!("Capsule: installing dependencies with pip...");
        run_checked(
            Command::new(py)
                .args(["-m", "pip", "install"])
                .arg("-q")
                .args(packages)
                .current_dir(&project.root),
        )
    }
}

fn freeze_selected(env: &Path, packages: &[String]) -> Result<BTreeMap<String, Package>> {
    let output = Command::new(python_bin(env))
        .args(["-m", "pip", "freeze", "--all"])
        .output()?;
    if !output.status.success() {
        bail!("pip freeze failed");
    }
    let wanted: BTreeSet<_> = packages.iter().map(|p| canonical(p)).collect();
    let original: BTreeMap<_, _> = packages.iter().map(|p| (canonical(p), p.clone())).collect();
    let mut versions = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some((name, version)) = line.split_once("==") {
            let key = canonical(name);
            if wanted.contains(&key) {
                versions.insert(
                    original[&key].clone(),
                    Package {
                        version: version.to_string(),
                    },
                );
            }
        }
    }
    let missing: Vec<_> = packages
        .iter()
        .filter(|package| !versions.contains_key(*package))
        .cloned()
        .collect();
    if !missing.is_empty() {
        bail!("could not freeze exact version(s): {}", missing.join(", "));
    }
    Ok(versions)
}

fn update_lock_with(env: &Path, project: &Project, packages: &[String]) -> Result<()> {
    let mut lock = lockfile::read(&project.root)?.unwrap_or_else(|| CapsuleLock {
        version: 1,
        project_hash: String::new(),
        language: project.language,
        runtime: python_runtime(),
        packages: BTreeMap::new(),
    });
    lock.packages.extend(freeze_selected(env, packages)?);
    lock.project_hash = crate::util::hashing::project_hash(
        &project.root,
        project.language,
        &lock.runtime.version,
        Some(&lock),
    )?;
    lockfile::write(&project.root, &lock)
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

fn run_python(env: &Path, entry: &Path) -> Result<RunResult> {
    let output = Command::new(python_bin(env)).arg(entry).output()?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(RunResult {
        code: output.status.code().unwrap_or(1),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn parse_module_not_found(stderr: &str) -> Option<String> {
    let re = Regex::new(r#"No module named ['"]([^'"]+)['"]"#).ok()?;
    Some(
        re.captures(stderr)?
            .get(1)?
            .as_str()
            .split('.')
            .next()?
            .to_string(),
    )
}

pub fn python_runtime() -> Runtime {
    let Some(py) = resolve_python_command() else {
        return Runtime {
            name: "python".into(),
            version: "unknown".into(),
        };
    };
    let output = Command::new(&py.program)
        .args(&py.args)
        .arg("--version")
        .output()
        .ok();
    let version = output
        .and_then(|out| {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if !stdout.is_empty() {
                Some(stdout)
            } else if !stderr.is_empty() {
                Some(stderr)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into())
        .replace("Python ", "");
    Runtime {
        name: py.display(),
        version,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PythonCommand {
    program: String,
    args: Vec<String>,
}

impl PythonCommand {
    fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

fn resolve_python_command() -> Option<PythonCommand> {
    let candidates = if cfg!(windows) {
        vec![
            PythonCommand {
                program: "py".into(),
                args: vec!["-3".into()],
            },
            PythonCommand {
                program: "python3".into(),
                args: Vec::new(),
            },
            PythonCommand {
                program: "python".into(),
                args: Vec::new(),
            },
        ]
    } else {
        vec![
            PythonCommand {
                program: "python3".into(),
                args: Vec::new(),
            },
            PythonCommand {
                program: "python".into(),
                args: Vec::new(),
            },
        ]
    };

    candidates.into_iter().find(|candidate| {
        Command::new(&candidate.program)
            .args(&candidate.args)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

fn python_bin(env: &Path) -> PathBuf {
    if cfg!(windows) {
        env.join("Scripts").join("python.exe")
    } else {
        env.join("bin").join("python")
    }
}

fn canonical(name: &str) -> String {
    name.to_lowercase().replace(['_', '.'], "-")
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
    use crate::adapters::Adapter;
    use crate::core::detector::{Language, Project};

    #[test]
    fn parses_module_not_found() {
        assert_eq!(
            parse_module_not_found("No module named 'requests.sessions'"),
            Some("requests".into())
        );
    }

    #[test]
    #[cfg(windows)]
    fn python_resolution_prefers_py_launcher_on_windows() {
        let expected = PythonCommand {
            program: "py".into(),
            args: vec!["-3".into()],
        };
        assert_eq!(expected.display(), "py -3");
    }

    #[test]
    #[cfg(unix)]
    fn python_resolution_prefers_python3_on_unix() {
        let expected = PythonCommand {
            program: "python3".into(),
            args: Vec::new(),
        };
        assert_eq!(expected.display(), "python3");
    }

    #[test]
    fn adapter_scans_real_file_with_inline_import() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.py");
        std::fs::write(
            &file,
            "\u{feff}import requests; print(requests.__version__)\n",
        )
        .unwrap();
        let project = Project {
            input: file.clone(),
            root: dir.path().to_path_buf(),
            language: Language::Python,
            entry: Some(file),
            is_file: true,
        };

        assert_eq!(PythonAdapter.scan(&project).unwrap(), vec!["requests"]);
    }
}
