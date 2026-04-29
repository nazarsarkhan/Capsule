use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::adapters::{node, python, typescript, Adapter};
use crate::core::detector::{detect, Language, Project};
use crate::core::lockfile::{self, CapsuleLock, Package, Runtime};
use crate::util::hashing::project_hash;

pub fn run_path(path: &Path, yes: bool, no_install: bool, verbose: bool) -> Result<()> {
    let project = detect(path)?;
    println!("Capsule: detected {} project", project.language.as_str());
    match project.language {
        Language::Python => python::PythonAdapter.run(&project, yes, no_install, verbose),
        Language::Node => node::NodeAdapter.run(&project, yes, no_install, verbose),
        Language::TypeScript => {
            typescript::TypeScriptAdapter.run(&project, yes, no_install, verbose)
        }
    }
}

pub fn scan_path(project: &Project) -> Result<()> {
    println!("Capsule: detected {} project", project.language.as_str());
    let packages = match project.language {
        Language::Python => python::PythonAdapter.scan(project)?,
        Language::Node => node::NodeAdapter.scan(project)?,
        Language::TypeScript => typescript::TypeScriptAdapter.scan(project)?,
    };
    println!("Capsule: packages: {}", fmt_packages(&packages));
    Ok(())
}

pub fn lock_path(path: &Path, yes: bool, no_install: bool, verbose: bool) -> Result<()> {
    let project = detect(path)?;
    println!("Capsule: detected {} project", project.language.as_str());
    match project.language {
        Language::Python => python::PythonAdapter.lock(&project, yes, no_install, verbose),
        Language::Node => node::NodeAdapter.lock(&project, yes, no_install, verbose),
        Language::TypeScript => {
            typescript::TypeScriptAdapter.lock(&project, yes, no_install, verbose)
        }
    }
}

pub fn base_lock(
    project: &Project,
    runtime: Runtime,
    packages: BTreeMap<String, Package>,
) -> Result<CapsuleLock> {
    let existing = lockfile::read(&project.root)?;
    let hash = project_hash(
        &project.root,
        project.language,
        runtime.version.as_str(),
        existing.as_ref(),
    )?;
    Ok(CapsuleLock {
        version: 1,
        project_hash: hash,
        language: project.language,
        runtime,
        packages,
    })
}

pub fn fmt_packages(packages: &[String]) -> String {
    if packages.is_empty() {
        "none".into()
    } else {
        packages.join(", ")
    }
}
