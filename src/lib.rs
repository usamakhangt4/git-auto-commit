pub mod cli;
pub mod config;
pub mod git;
pub mod ollama;
pub mod prompt;

pub use cli::{run_with, Cli, Commands};

pub use cli::run;

pub const MAX_PROMPT_CHARS: usize = 8_000;
pub const BOOTSTRAP_FILE_THRESHOLD: usize = 30;
pub const MAX_LINES_FOR_FULL_DIFF: u32 = 150;
pub const MAX_FULL_DIFF_FILES: usize = 8;
