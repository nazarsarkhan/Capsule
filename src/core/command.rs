use std::process::Command;

pub fn command_exists(name: &str) -> bool {
    resolve_command(name).is_some()
}

pub fn resolve_command(name: &str) -> Option<String> {
    let primary = platform_command_name(name);
    let mut candidates = vec![primary];
    if cfg!(windows) && matches!(name, "pnpm" | "npm" | "npx") {
        candidates.push(name.to_string());
    }
    candidates
        .into_iter()
        .find(|candidate| raw_command_exists(candidate))
}

pub fn platform_command_name(name: &str) -> String {
    if cfg!(windows) {
        match name {
            "pnpm" | "npm" | "npx" => format!("{name}.cmd"),
            _ => name.to_string(),
        }
    } else {
        name.to_string()
    }
}

fn raw_command_exists(name: &str) -> bool {
    let check = if cfg!(windows) { "where" } else { "which" };
    Command::new(check)
        .arg(name)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn command_output(name: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(name).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        None
    } else {
        Some(stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn node_package_managers_use_cmd_shims_on_windows() {
        assert_eq!(platform_command_name("pnpm"), "pnpm.cmd");
        assert_eq!(platform_command_name("npm"), "npm.cmd");
        assert_eq!(platform_command_name("npx"), "npx.cmd");
    }

    #[test]
    #[cfg(unix)]
    fn node_package_managers_use_plain_names_on_unix() {
        assert_eq!(platform_command_name("pnpm"), "pnpm");
        assert_eq!(platform_command_name("npm"), "npm");
        assert_eq!(platform_command_name("npx"), "npx");
    }
}
