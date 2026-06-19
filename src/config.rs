use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const CONFIG_DIR_ENV: &str = "GIT_AUTO_COMMIT_CONFIG_DIR";
const FORMAT_ENV: &str = "COMMIT_FORMAT";
const TIMEOUT_ENV: &str = "GIT_AUTO_COMMIT_TIMEOUT_SECS";
const DEFAULT_FORMAT: &str = "type(scope): description";
const DEFAULT_TIMEOUT_SECS: u64 = 300;

pub fn config_dir() -> Result<PathBuf> {
    if let Ok(path) = env::var(CONFIG_DIR_ENV) {
        if path.is_empty() {
            bail!("{CONFIG_DIR_ENV} must not be empty");
        }
        return Ok(PathBuf::from(path));
    }

    let mut path = dirs::config_dir().context("could not find config directory")?;
    path.push("git-auto-commit");
    Ok(path)
}

pub fn get_setting(key: &str, default: &str) -> String {
    let path = match config_dir() {
        Ok(p) => p.join(format!("{key}.txt")),
        Err(e) => {
            eprintln!("Warning: {e:#}");
            return default.to_string();
        }
    };
    fs::read_to_string(path)
        .unwrap_or_else(|_| default.to_string())
        .trim()
        .to_string()
}

/// Returns the commit message format: `format.txt`, then `COMMIT_FORMAT`, then the default.
pub fn get_format_spec() -> String {
    let path = match config_dir() {
        Ok(p) => p.join("format.txt"),
        Err(e) => {
            eprintln!("Warning: {e:#}");
            return format_from_env_or_default();
        }
    };

    match fs::read_to_string(&path) {
        Ok(content) => content.trim().to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => format_from_env_or_default(),
        Err(e) => {
            eprintln!("Warning: failed to read {}: {e:#}", path.display());
            DEFAULT_FORMAT.to_string()
        }
    }
}

fn format_from_env_or_default() -> String {
    env::var(FORMAT_ENV)
        .unwrap_or_else(|_| DEFAULT_FORMAT.to_string())
        .trim()
        .to_string()
}

pub fn request_timeout() -> Duration {
    env::var(TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS))
}

pub fn load_ignore_patterns() -> Vec<String> {
    let path = match config_dir() {
        Ok(p) => p.join("ignore.txt"),
        Err(e) => {
            eprintln!("Warning: {e:#}");
            return default_ignore_patterns();
        }
    };
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut patterns: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect();
    if patterns.is_empty() {
        patterns = default_ignore_patterns();
    }
    patterns
}

pub fn default_ignore_patterns() -> Vec<String> {
    vec![
        "target/".into(),
        "node_modules/".into(),
        "dist/".into(),
        ".next/".into(),
        "vendor/".into(),
        "package-lock.json".into(),
        "yarn.lock".into(),
        "pnpm-lock.yaml".into(),
        "Cargo.lock".into(),
        "*.min.js".into(),
        "*.min.css".into(),
        "*.map".into(),
        "*.png".into(),
        "*.jpg".into(),
        "*.jpeg".into(),
        "*.gif".into(),
        "*.ico".into(),
        "*.webp".into(),
        "*.woff".into(),
        "*.woff2".into(),
        "*.ttf".into(),
        "*.pdf".into(),
        "*.zip".into(),
    ]
}

pub fn load_model_name() -> Result<Option<String>> {
    let path = config_dir()?.join("model.txt");
    match fs::read_to_string(&path) {
        Ok(content) => {
            let trimmed = content.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("failed to read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvOverride {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvOverride {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvOverride {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn with_config_dir(path: &Path) -> EnvOverride {
        EnvOverride::set(CONFIG_DIR_ENV, &path.to_string_lossy())
    }

    #[test]
    fn default_ignore_patterns_is_non_empty() {
        assert!(!default_ignore_patterns().is_empty());
    }

    #[test]
    fn format_falls_back_to_commit_format_env() {
        let _lock = ENV_LOCK.lock().expect("environment lock");
        let config_dir = TempDir::new().expect("tempdir");
        let _dir = with_config_dir(config_dir.path());
        let _format = EnvOverride::set(FORMAT_ENV, "custom: env");

        assert_eq!(get_format_spec(), "custom: env");
    }

    #[test]
    fn format_file_takes_precedence_over_env() {
        let _lock = ENV_LOCK.lock().expect("environment lock");
        let config_dir = TempDir::new().expect("tempdir");
        let _dir = with_config_dir(config_dir.path());
        let _format = EnvOverride::set(FORMAT_ENV, "from env");
        std::fs::write(config_dir.path().join("format.txt"), "from file").expect("write");

        assert_eq!(get_format_spec(), "from file");
    }

    #[test]
    fn format_uses_default_when_no_file_or_env() {
        let _lock = ENV_LOCK.lock().expect("environment lock");
        let config_dir = TempDir::new().expect("tempdir");
        let _dir = with_config_dir(config_dir.path());
        let _format = EnvOverride::unset(FORMAT_ENV);

        assert_eq!(get_format_spec(), DEFAULT_FORMAT);
    }
}
