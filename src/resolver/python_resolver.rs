pub fn resolve(import: &str) -> String {
    match import {
        "PIL" => "Pillow",
        "cv2" => "opencv-python",
        "sklearn" => "scikit-learn",
        "yaml" => "PyYAML",
        "bs4" => "beautifulsoup4",
        "dotenv" => "python-dotenv",
        "Crypto" => "pycryptodome",
        "dateutil" => "python-dateutil",
        "jwt" => "PyJWT",
        "lxml" => "lxml",
        "requests" => "requests",
        "fastapi" => "fastapi",
        "pandas" => "pandas",
        "numpy" => "numpy",
        "openai" => "openai",
        "flask" => "flask",
        "django" => "django",
        "rich" => "rich",
        "typer" => "typer",
        other => other,
    }
    .to_string()
}

pub fn is_curated(import: &str) -> bool {
    resolve(import) != import
        || matches!(
            import,
            "lxml"
                | "requests"
                | "fastapi"
                | "pandas"
                | "numpy"
                | "openai"
                | "flask"
                | "django"
                | "rich"
                | "typer"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_import_names() {
        assert_eq!(resolve("PIL"), "Pillow");
        assert_eq!(resolve("cv2"), "opencv-python");
        assert_eq!(resolve("sklearn"), "scikit-learn");
    }
}
