use std::path::Path;

use anyhow::Result;

use crate::adapters::{node, python, Adapter};
use crate::core::cache;
use crate::core::detector::{detect, Language, Project};
use crate::core::lockfile;
use crate::scanners::node_scanner;
use crate::util::hashing::project_hash;

pub fn inspect_path(path: &Path) -> Result<()> {
    let project = detect(path)?;
    let lock = lockfile::read(&project.root)?;
    let runtime = runtime_for_project(&project);
    let hash = project_hash(
        &project.root,
        project.language,
        runtime.version.as_str(),
        lock.as_ref(),
    )?;
    let cache_path = cache::env_path_no_create(project.language, &hash)?;
    let direct = direct_packages(&project)?;
    let runtime_packages = runtime_packages(&project);

    println!("Capsule Inspect");
    println!("---------------");
    println!("language: {}", project.language.as_str());
    println!(
        "entry: {}",
        project
            .entry
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| project.root.display().to_string())
    );
    println!("packages (direct):");
    print_list(&direct);
    println!("runtime packages:");
    print_list(&runtime_packages);
    println!("cache:");
    println!("  path: {}", cache_path.display());
    println!("lockfile:");
    if let Some(lock) = lock {
        println!("  present: yes");
        println!("  packages:");
        for (name, package) in lock.packages {
            println!("    {name}@{}", package.version);
        }
    } else {
        println!("  present: no");
    }
    Ok(())
}

fn runtime_for_project(project: &Project) -> lockfile::Runtime {
    match project.language {
        Language::Python => python::python_runtime(),
        Language::Node | Language::TypeScript => node::node_runtime(),
    }
}

fn direct_packages(project: &Project) -> Result<Vec<String>> {
    match project.language {
        Language::Python => python::PythonAdapter.scan(project),
        Language::Node => node::NodeAdapter.scan(project),
        Language::TypeScript => node_scanner::scan_path(&project.input),
    }
}

fn runtime_packages(project: &Project) -> Vec<String> {
    match project.language {
        Language::TypeScript => vec!["tsx".to_string()],
        Language::Python | Language::Node => Vec::new(),
    }
}

fn print_list(values: &[String]) {
    if values.is_empty() {
        println!("  none");
    } else {
        for value in values {
            println!("  - {value}");
        }
    }
}
