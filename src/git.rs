use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown,
}

impl FileStatus {
    pub fn from_git_token(token: &str) -> Self {
        match token.chars().next() {
            Some('A') => Self::Added,
            Some('M') => Self::Modified,
            Some('D') => Self::Deleted,
            Some('R') => Self::Renamed,
            Some('C') => Self::Copied,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StagedFile {
    pub status: FileStatus,
    pub path: String,
    pub added: u32,
    pub deleted: u32,
}

pub fn run_git(args: &[&str]) -> Result<String> {
    let cmd = args.join(" ");
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {cmd}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            bail!("git {cmd} failed");
        } else {
            bail!("git {cmd} failed: {detail}");
        }
    }
    String::from_utf8(output.stdout).context("git output was not valid UTF-8")
}

pub fn parse_name_status(raw: &str) -> Vec<StagedFile> {
    let mut files = Vec::new();
    for line in raw.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let status_token = parts.next().unwrap_or("");
        let status = FileStatus::from_git_token(status_token);
        let path = if matches!(status, FileStatus::Renamed | FileStatus::Copied) {
            parts.nth(1).unwrap_or("").to_string()
        } else {
            parts.next().unwrap_or("").to_string()
        };
        if path.is_empty() {
            continue;
        }
        files.push(StagedFile {
            status,
            path,
            added: 0,
            deleted: 0,
        });
    }
    files
}

pub fn parse_numstat(raw: &str) -> HashMap<String, (u32, u32)> {
    let mut stats = HashMap::new();
    for (line_no, line) in raw.lines().enumerate() {
        let mut parts = line.split('\t');
        let added_raw = parts.next().unwrap_or("0");
        let deleted_raw = parts.next().unwrap_or("0");
        let path = parts.next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        let added = parse_stat_field(added_raw, line_no + 1, "added");
        let deleted = parse_stat_field(deleted_raw, line_no + 1, "deleted");
        stats.insert(path.to_string(), (added, deleted));
    }
    stats
}

fn parse_stat_field(raw: &str, line_no: usize, field: &str) -> u32 {
    if raw == "-" {
        return 0;
    }
    match raw.parse() {
        Ok(n) => n,
        Err(_) => {
            eprintln!(
                "Warning: invalid {field} value '{raw}' on numstat line {line_no}, treating as 0"
            );
            0
        }
    }
}

pub fn is_inside_work_tree() -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .context("failed to check git repository")?;
    Ok(output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_status_handles_rename() {
        let raw = "R100\told/path.rs\tnew/path.rs\nA\tREADME.md\n";
        let files = parse_name_status(raw);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "new/path.rs");
        assert_eq!(files[0].status, FileStatus::Renamed);
        assert_eq!(files[1].path, "README.md");
        assert_eq!(files[1].status, FileStatus::Added);
    }

    #[test]
    fn parse_numstat_parses_additions_and_deletions() {
        let raw = "10\t5\tsrc/main.rs\n-\t-\tbinary.png\n";
        let stats = parse_numstat(raw);
        assert_eq!(stats.get("src/main.rs"), Some(&(10, 5)));
        assert_eq!(stats.get("binary.png"), Some(&(0, 0)));
    }

    #[test]
    fn parse_numstat_warns_on_invalid_values() {
        let raw = "abc\t2\tbroken.rs\n";
        let stats = parse_numstat(raw);
        assert_eq!(stats.get("broken.rs"), Some(&(0, 2)));
    }

    #[test]
    fn file_status_labels() {
        assert_eq!(FileStatus::Added.label(), "A");
        assert_eq!(FileStatus::Unknown.label(), "?");
    }
}
