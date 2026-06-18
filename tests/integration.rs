mod common;

use clap::Parser;
use common::{outside_git_repo, with_outside_repo_cwd, ConfigOverride, GitRepo, TestIsolation};
use git_auto_commit::prompt::build_commit_context;
use git_auto_commit::{run_with, Cli};
use tempfile::TempDir;

#[test]
fn build_commit_context_lists_staged_files() {
    let repo = GitRepo::new();
    repo.write_and_stage("hello.txt", "hello world\n");

    let context = repo.in_repo(|| build_commit_context().expect("build context"));
    assert!(context.text.contains("hello.txt"));
    assert!(context.text.contains("Staged changes: 1 files"));
}

#[test]
fn build_commit_context_is_empty_without_staged_changes() {
    let repo = GitRepo::new();

    let context = repo.in_repo(|| build_commit_context().expect("build context"));
    assert!(context.text.trim().is_empty());
}

#[test]
fn set_model_writes_config_file() {
    let _iso = TestIsolation::new();
    let config_dir = TempDir::new().expect("config tempdir");
    let _override = ConfigOverride::new(config_dir.path());

    with_outside_repo_cwd(|| {
        let cli = Cli::try_parse_from(["git-auto-commit", "set-model", "llama3"]).expect("parse");
        run_with(cli).expect("set model");
    });

    let saved = std::fs::read_to_string(config_dir.path().join("model.txt")).expect("read");
    assert_eq!(saved, "llama3");
}

#[test]
fn set_format_writes_config_file() {
    let _iso = TestIsolation::new();
    let config_dir = TempDir::new().expect("config tempdir");
    let _override = ConfigOverride::new(config_dir.path());

    with_outside_repo_cwd(|| {
        let cli =
            Cli::try_parse_from(["git-auto-commit", "set-format", "type(scope): description"])
                .expect("parse");
        run_with(cli).expect("set format");
    });

    let saved = std::fs::read_to_string(config_dir.path().join("format.txt")).expect("read");
    assert_eq!(saved, "type(scope): description");
}

#[test]
fn config_commands_work_outside_git_repository() {
    let _iso = TestIsolation::new();
    let config_dir = TempDir::new().expect("config tempdir");
    let _override = ConfigOverride::new(config_dir.path());

    with_outside_repo_cwd(|| {
        let model_cli =
            Cli::try_parse_from(["git-auto-commit", "set-model", "qwen"]).expect("parse");
        run_with(model_cli).expect("set model outside repo");

        let format_cli =
            Cli::try_parse_from(["git-auto-commit", "set-format", "chore: outside repo"])
                .expect("parse");
        run_with(format_cli).expect("set format outside repo");
    });

    assert_eq!(
        std::fs::read_to_string(config_dir.path().join("model.txt")).expect("read"),
        "qwen"
    );
    assert_eq!(
        std::fs::read_to_string(config_dir.path().join("format.txt")).expect("read"),
        "chore: outside repo"
    );
}

#[test]
fn run_fails_outside_git_repository() {
    outside_git_repo(|| {
        let cli = Cli::try_parse_from(["git-auto-commit"]).expect("parse");
        let err = run_with(cli).expect_err("expected failure outside git repo");
        assert!(err.to_string().contains("not inside a git repository"));
    });
}

#[test]
fn load_model_name_ignores_whitespace_only_config() {
    let _repo = GitRepo::new();
    let config_dir = TempDir::new().expect("config tempdir");
    let _override = ConfigOverride::new(config_dir.path());
    std::fs::write(config_dir.path().join("model.txt"), "  \n").expect("write");

    let model = git_auto_commit::config::load_model_name().expect("load");
    assert_eq!(model, None);
}
