pub fn package_root(specifier: &str) -> String {
    if specifier.starts_with('@') {
        let mut parts = specifier.split('/');
        let scope = parts.next().unwrap_or_default();
        let pkg = parts.next().unwrap_or_default();
        format!("{scope}/{pkg}")
    } else {
        specifier.split('/').next().unwrap_or(specifier).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_node_subpaths() {
        assert_eq!(package_root("lodash/debounce"), "lodash");
        assert_eq!(package_root("@scope/pkg/sub/path"), "@scope/pkg");
    }
}
