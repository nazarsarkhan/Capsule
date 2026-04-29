use anyhow::{bail, Result};

pub fn suspicious_package_name(name: &str) -> bool {
    if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_whitespace) {
        return true;
    }
    if name.contains("..") {
        return true;
    }
    if name
        .chars()
        .any(|c| matches!(c, ';' | '&' | '|' | '`' | '$' | '<' | '>' | '(' | ')'))
    {
        return true;
    }
    if name.contains('\\') {
        return true;
    }
    if name.contains('/') {
        return !valid_scoped_npm(name);
    }
    false
}

fn valid_scoped_npm(name: &str) -> bool {
    let mut parts = name.split('/');
    let scope = parts.next().unwrap_or_default();
    let pkg = parts.next().unwrap_or_default();
    parts.next().is_none() && scope.starts_with('@') && scope.len() > 1 && !pkg.is_empty()
}

pub fn validate_packages(packages: &[String]) -> Result<()> {
    let bad: Vec<_> = packages
        .iter()
        .filter(|p| suspicious_package_name(p))
        .cloned()
        .collect();
    if !bad.is_empty() {
        bail!("suspicious package name(s): {}", bad.join(", "));
    }
    Ok(())
}

pub fn confirm_unknown(packages: &[String], yes: bool) -> Result<()> {
    validate_packages(packages)?;
    if packages.is_empty() || yes {
        return Ok(());
    }
    println!(
        "Capsule: warning: unknown packages: {}",
        packages.join(", ")
    );
    println!("Capsule: install inferred unknown packages? [y/N]");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        bail!("installation cancelled")
    }
}

pub fn report_unknown_no_install(packages: &[String]) -> Result<()> {
    validate_packages(packages)?;
    if !packages.is_empty() {
        println!(
            "Capsule: warning: unknown packages: {}",
            packages.join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_suspicious_names() {
        assert!(suspicious_package_name(""));
        assert!(suspicious_package_name("-bad"));
        assert!(suspicious_package_name("bad;rm"));
        assert!(suspicious_package_name("../bad"));
        assert!(!suspicious_package_name("@scope/pkg"));
        assert!(!suspicious_package_name("requests"));
    }
}
