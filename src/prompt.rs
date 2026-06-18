use crate::config::load_ignore_patterns;
use crate::git::{parse_name_status, parse_numstat, run_git, FileStatus, StagedFile};
use crate::{
    BOOTSTRAP_FILE_THRESHOLD, MAX_FULL_DIFF_FILES, MAX_LINES_FOR_FULL_DIFF, MAX_PROMPT_CHARS,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fmt::Write as _;

pub fn pattern_matches(path: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        if let Some(suffix) = pattern.strip_prefix('*') {
            return path.ends_with(suffix);
        }
    }
    if pattern.ends_with('/') {
        let suffix = pattern.trim_end_matches('/');
        return path.starts_with(pattern) || path.split('/').any(|segment| segment == suffix);
    }
    path == pattern || path.ends_with(&format!("/{pattern}"))
}

pub fn is_ignored(path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| pattern_matches(path, p))
}

pub fn is_binary_stat(added: u32, deleted: u32) -> bool {
    added == 0 && deleted == 0
}

pub fn is_key_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        name,
        "Cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "go.mod"
            | "README.md"
            | "README"
            | "LICENSE"
            | "Makefile"
            | "Dockerfile"
            | "compose.yaml"
            | "docker-compose.yml"
    ) || path.ends_with("/main.rs")
        || path.ends_with("/lib.rs")
        || path.ends_with("/index.ts")
        || path.ends_with("/index.js")
        || path.ends_with("/mod.rs")
}

pub fn is_source_file(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    matches!(
        ext,
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "rb"
            | "php"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "vue"
            | "svelte"
            | "toml"
            | "yaml"
            | "yml"
            | "json"
            | "md"
            | "sql"
            | "sh"
    )
}

pub fn should_include_full_diff(file: &StagedFile, ignored: bool) -> bool {
    if ignored || is_binary_stat(file.added, file.deleted) {
        return false;
    }
    if is_key_file(&file.path) {
        return file.added + file.deleted <= MAX_LINES_FOR_FULL_DIFF;
    }
    if !is_source_file(&file.path) {
        return false;
    }
    file.added + file.deleted <= MAX_LINES_FOR_FULL_DIFF
}

pub fn is_bootstrap_commit(files: &[StagedFile]) -> bool {
    if files.len() < BOOTSTRAP_FILE_THRESHOLD {
        return false;
    }
    let added = files
        .iter()
        .filter(|f| f.status == FileStatus::Added)
        .count();
    added.saturating_mul(100) / files.len().max(1) >= 90
}

fn summarize_directories(paths: &[&str]) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for path in paths {
        let dir = path
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_else(|| ".".to_string());
        *counts.entry(dir).or_insert(0) += 1;
    }
    let mut dirs: Vec<_> = counts.into_iter().collect();
    dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    dirs
}

fn write_file_summary(prompt: &mut String, file: &StagedFile, ignored: bool) {
    let status = file.status.label();
    if ignored {
        let _ = writeln!(prompt, "{status}  {} (ignored, content omitted)", file.path);
        return;
    }
    if is_binary_stat(file.added, file.deleted) {
        let _ = writeln!(prompt, "{status}  {} (binary, content omitted)", file.path);
        return;
    }
    if file.added + file.deleted > MAX_LINES_FOR_FULL_DIFF {
        let _ = writeln!(
            prompt,
            "{status}  {} (+{} / -{}, content omitted)",
            file.path, file.added, file.deleted
        );
        return;
    }
    let _ = writeln!(
        prompt,
        "{status}  {} (+{} / -{})",
        file.path, file.added, file.deleted
    );
}

pub fn build_commit_context() -> Result<CommitContext> {
    build_commit_context_with_options(CommitContextOptions {
        include_diffs: true,
    })
}

pub fn build_commit_context_summary() -> Result<String> {
    build_commit_context_with_options(CommitContextOptions {
        include_diffs: false,
    })
    .map(|ctx| ctx.text)
}

#[derive(Debug, Clone)]
pub struct CommitContext {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CommitContextOptions {
    pub include_diffs: bool,
}

pub fn build_commit_context_with_options(options: CommitContextOptions) -> Result<CommitContext> {
    let name_status =
        run_git(&["diff", "--staged", "--name-status"]).context("failed to list staged files")?;
    let mut files = parse_name_status(&name_status);
    if files.is_empty() {
        return Ok(CommitContext {
            text: String::new(),
            truncated: false,
        });
    }

    let numstat =
        run_git(&["diff", "--staged", "--numstat"]).context("failed to read staged diff stats")?;
    let stats = parse_numstat(&numstat);
    for file in &mut files {
        if let Some((added, deleted)) = stats.get(&file.path) {
            file.added = *added;
            file.deleted = *deleted;
        }
    }

    let ignore_patterns = load_ignore_patterns();
    let ignored_flags: Vec<bool> = files
        .iter()
        .map(|f| is_ignored(&f.path, &ignore_patterns))
        .collect();

    let total_added: u32 = files.iter().map(|f| f.added).sum();
    let total_deleted: u32 = files.iter().map(|f| f.deleted).sum();
    let is_bootstrap = is_bootstrap_commit(&files);

    let estimated_len = files.len() * 96 + 256;
    let mut prompt = String::with_capacity(estimated_len);
    let _ = writeln!(
        prompt,
        "Staged changes: {} files (+{} / -{})\n",
        files.len(),
        total_added,
        total_deleted
    );

    if is_bootstrap {
        prompt.push_str(
            "Bulk/initial commit — infer intent from paths and key files, not every file.\n\n",
        );
        prompt.push_str("Directories:\n");
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        for (dir, count) in summarize_directories(&paths).into_iter().take(15) {
            let _ = writeln!(prompt, "  {dir}/  ({count} files)");
        }
        prompt.push('\n');
    }

    prompt.push_str("Files:\n");
    for (file, ignored) in files.iter().zip(ignored_flags.iter().copied()) {
        write_file_summary(&mut prompt, file, ignored);
    }

    let mut diff_candidates: Vec<&StagedFile> = files
        .iter()
        .zip(ignored_flags.iter().copied())
        .filter_map(|(file, ignored)| should_include_full_diff(file, ignored).then_some(file))
        .collect();

    diff_candidates.sort_by(|a, b| {
        let a_key = is_key_file(&a.path) as u8;
        let b_key = is_key_file(&b.path) as u8;
        b_key
            .cmp(&a_key)
            .then_with(|| (a.added + a.deleted).cmp(&(b.added + b.deleted)))
    });
    diff_candidates.truncate(MAX_FULL_DIFF_FILES);

    if options.include_diffs && !diff_candidates.is_empty() {
        prompt.push_str("\nDetailed diffs (selected files only):\n");
        let mut diff_args: Vec<&str> = vec!["diff", "--staged", "--"];
        for file in &diff_candidates {
            diff_args.push(file.path.as_str());
        }
        let combined_diff = run_git(&diff_args).context("failed to read staged file diffs")?;
        if !combined_diff.trim().is_empty() {
            prompt.push('\n');
            prompt.push_str(&combined_diff);
            if !combined_diff.ends_with('\n') {
                prompt.push('\n');
            }
        }
    }

    let mut truncated = false;
    if prompt.len() > MAX_PROMPT_CHARS {
        truncated = true;
        eprintln!(
            "Warning: prompt truncated to {} chars (was {})",
            MAX_PROMPT_CHARS,
            prompt.len()
        );
        truncate_to_byte_limit(&mut prompt, MAX_PROMPT_CHARS);
    }

    Ok(CommitContext {
        text: prompt,
        truncated,
    })
}

/// Truncates `text` to at most `max_bytes` without splitting a UTF-8 codepoint.
/// Prefers cutting at the last newline in the second half of the range so diffs
/// are not left mid-line.
fn truncate_to_byte_limit(text: &mut String, max_bytes: usize) {
    if text.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    if let Some(newline) = text[..end].rfind('\n') {
        if newline >= max_bytes / 2 {
            end = newline;
        }
    }
    text.truncate(end);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::FileStatus;

    fn staged(path: &str, status: FileStatus, added: u32, deleted: u32) -> StagedFile {
        StagedFile {
            status,
            path: path.to_string(),
            added,
            deleted,
        }
    }

    #[test]
    fn pattern_matches_globs_and_dirs() {
        assert!(pattern_matches(
            "foo/package-lock.json",
            "package-lock.json"
        ));
        assert!(pattern_matches("target/debug/foo", "target/"));
        assert!(pattern_matches("app.min.js", "*.min.js"));
    }

    #[test]
    fn is_bootstrap_commit_detects_bulk_adds() {
        let files: Vec<StagedFile> = (0..35)
            .map(|i| staged(&format!("src/file_{i}.rs"), FileStatus::Added, 10, 0))
            .collect();
        assert!(is_bootstrap_commit(&files));
    }

    #[test]
    fn is_bootstrap_commit_rejects_small_changesets() {
        let files = vec![staged("src/main.rs", FileStatus::Modified, 3, 1)];
        assert!(!is_bootstrap_commit(&files));
    }

    #[test]
    fn should_include_full_diff_for_small_source_file() {
        let file = staged("src/main.rs", FileStatus::Modified, 10, 5);
        assert!(should_include_full_diff(&file, false));
    }

    #[test]
    fn should_exclude_ignored_and_large_files() {
        let ignored = staged("src/main.rs", FileStatus::Modified, 10, 5);
        assert!(!should_include_full_diff(&ignored, true));

        let large = staged("src/main.rs", FileStatus::Modified, 200, 0);
        assert!(!should_include_full_diff(&large, false));
    }

    #[test]
    fn is_ignored_respects_patterns() {
        let patterns = vec!["target/".to_string(), "*.min.js".to_string()];
        assert!(is_ignored("target/debug/foo", &patterns));
        assert!(is_ignored("dist/app.min.js", &patterns));
        assert!(!is_ignored("src/main.rs", &patterns));
    }

    #[test]
    fn truncate_to_byte_limit_avoids_splitting_multibyte_chars() {
        // Each emoji is 4 bytes; byte 8_000 falls inside the second emoji.
        let mut prompt = "x".repeat(MAX_PROMPT_CHARS - 5);
        prompt.push_str("🎉🎉🎉");

        truncate_to_byte_limit(&mut prompt, MAX_PROMPT_CHARS);

        assert!(prompt.len() <= MAX_PROMPT_CHARS);
        assert!(std::str::from_utf8(prompt.as_bytes()).is_ok());
    }

    #[test]
    #[should_panic]
    fn raw_truncate_panics_on_multibyte_boundary() {
        let mut prompt = "x".repeat(MAX_PROMPT_CHARS - 5);
        prompt.push_str("🎉🎉🎉");
        prompt.truncate(MAX_PROMPT_CHARS);
    }
}
