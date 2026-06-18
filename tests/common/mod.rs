use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Serializes tests that mutate process cwd or environment variables.
pub struct TestIsolation {
    _lock: MutexGuard<'static, ()>,
}

impl TestIsolation {
    pub fn new() -> Self {
        Self { _lock: test_lock() }
    }
}

/// Temporary git repository for integration tests.
pub struct GitRepo {
    _lock: MutexGuard<'static, ()>,
    dir: TempDir,
}

impl GitRepo {
    pub fn new() -> Self {
        let lock = test_lock();
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();

        run_git_in(root, &["init"]);
        run_git_in(
            root,
            &["config", "user.email", "git-auto-commit@test.example"],
        );
        run_git_in(root, &["config", "user.name", "git-auto-commit"]);

        Self { _lock: lock, dir }
    }

    pub fn write_and_stage(&self, rel_path: &str, content: &str) {
        let file = self.dir.path().join(rel_path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(&file, content).expect("write file");
        run_git_in(self.dir.path(), &["add", rel_path]);
    }

    /// Run a closure with the process cwd set to this repository.
    pub fn in_repo<T>(&self, f: impl FnOnce() -> T) -> T {
        let previous = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(self.dir.path()).expect("chdir into repo");
        let result = f();
        std::env::set_current_dir(previous).expect("restore cwd");
        result
    }
}

/// Overrides `GIT_AUTO_COMMIT_CONFIG_DIR` for the duration of a test.
pub struct ConfigOverride {
    previous: Option<String>,
}

impl ConfigOverride {
    pub fn new(path: &Path) -> Self {
        let previous = std::env::var("GIT_AUTO_COMMIT_CONFIG_DIR").ok();
        std::env::set_var("GIT_AUTO_COMMIT_CONFIG_DIR", path);
        Self { previous }
    }
}

impl Drop for ConfigOverride {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("GIT_AUTO_COMMIT_CONFIG_DIR", value),
            None => std::env::remove_var("GIT_AUTO_COMMIT_CONFIG_DIR"),
        }
    }
}

/// Run a closure with cwd set outside any git repository.
///
/// The caller must hold [`TestIsolation`] (or a [`GitRepo`]) for the whole test.
pub fn with_outside_repo_cwd<T>(f: impl FnOnce() -> T) -> T {
    let dir = TempDir::new().expect("tempdir");
    let previous = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("chdir outside repo");
    let result = f();
    std::env::set_current_dir(previous).expect("restore cwd");
    result
}

/// Run a closure outside any git repository (isolated from parallel tests).
pub fn outside_git_repo<T>(f: impl FnOnce() -> T) -> T {
    let _iso = TestIsolation::new();
    with_outside_repo_cwd(f)
}

fn run_git_in(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo);
    command.args(args);
    let status = command.status().expect("spawn git");
    assert!(
        status.success(),
        "git -C {} {} failed",
        repo.display(),
        args.join(" ")
    );
}
