/// Hidden directories the model may list, search, and read.
pub const ALLOW_HIDDEN_NAMES: &[&str] = &[".github", ".agents"];

/// Dependency, cache, VCS, and build directories. Never walked.
pub const DENY_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "target",
    "dist",
    "build",
    "out",
    "output",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".venv",
    "venv",
    ".tox",
    "env",
    "vendor",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".cache",
    ".parcel-cache",
    "coverage",
    ".nyc_output",
    "bin",
    "obj",
    "Pods",
    ".gradle",
    "elm-stuff",
    "_build",
    "deps",
    ".stack-work",
    ".pnpm",
    ".yarn",
    "bower_components",
    "jspm_packages",
    ".serverless",
    ".terraform",
    ".cargo",
    "zig-cache",
    "zig-out",
    ".dart_tool",
    "DerivedData",
    "Carthage",
    ".bundle",
    "htmlcov",
    ".hypothesis",
    "site-packages",
    ".mvn",
    ".idea",
    ".vscode",
    ".vs",
    ".local",
    ".config",
    "snap",
    ".var",
    ".mozilla",
    ".thunderbird",
    ".npm",
    ".nvm",
    "go",
];

pub fn skip_entry(name: &str) -> bool {
    if ALLOW_HIDDEN_NAMES
        .iter()
        .any(|item| name.eq_ignore_ascii_case(item))
    {
        return false;
    }
    if name.starts_with('.') {
        return true;
    }
    DENY_DIR_NAMES
        .iter()
        .any(|item| name.eq_ignore_ascii_case(item))
}

#[cfg(test)]
mod tests {
    use super::skip_entry;

    #[test]
    fn allows_project_config_and_blocks_dependencies() {
        assert!(!skip_entry(".github"));
        assert!(!skip_entry(".agents"));
        assert!(!skip_entry("src"));
        assert!(skip_entry("node_modules"));
        assert!(skip_entry(".git"));
        assert!(skip_entry(".env"));
        assert!(skip_entry("target"));
    }
}
