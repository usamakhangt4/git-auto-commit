use clap::{Parser, Subcommand};

use crate::config::{config_dir, get_format_spec, load_model_name};
use crate::git::is_inside_work_tree;
use crate::ollama::{discover_model, generate_commit_message, ollama_host};
use crate::prompt::build_commit_context;
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::{self, Write};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "git-auto-commit",
    version,
    about = "AI-powered commit message generator using Ollama"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Set the Ollama model to use
    SetModel {
        /// Ollama model name (e.g. llama3)
        model: String,
    },
    /// Set the commit message format instruction for the model
    SetFormat {
        /// Format description (e.g. "type(scope): description")
        format: String,
    },
}

pub fn run() -> Result<()> {
    run_with(Cli::parse())
}

pub fn run_with(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::SetFormat { format }) => set_format(&format),
        Some(Commands::SetModel { model }) => set_model(&model),
        None => {
            if !is_inside_work_tree()? {
                bail!("not inside a git repository");
            }
            run_commit_flow()
        }
    }
}

fn set_format(format: &str) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).context("failed to create config directory")?;
    fs::write(dir.join("format.txt"), format).context("failed to write format.txt")?;
    println!("Format set to: {format}");
    Ok(())
}

fn set_model(model: &str) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).context("failed to create config directory")?;
    fs::write(dir.join("model.txt"), model).context("failed to write model.txt")?;
    println!("Model set to: {model}");
    Ok(())
}

fn run_commit_flow() -> Result<()> {
    let host = ollama_host();
    let prompt = build_commit_context()?;

    if prompt.trim().is_empty() {
        println!("No staged changes found. Use 'git add' to stage files first.");
        return Ok(());
    }

    let format_spec = get_format_spec();

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    let target_model = match load_model_name()? {
        Some(model) => model,
        None => discover_model(&client, &host)?,
    };

    println!("Using model: {target_model} | Format: {format_spec}");

    let commit_message =
        generate_commit_message(&client, &host, &target_model, &format_spec, &prompt)?;

    println!("\nGenerated Message:\n>>> {commit_message}");
    print!("\nCommit? [y/N]: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read confirmation")?;

    if confirms_commit(&input) {
        let status = Command::new("git")
            .args(["commit", "-m", &commit_message])
            .status()
            .context("failed to run git commit")?;
        if status.success() {
            println!("Successfully committed!");
        } else {
            bail!("git commit failed");
        }
    } else {
        println!("Aborted.");
    }

    Ok(())
}

pub(crate) fn confirms_commit(input: &str) -> bool {
    input.trim().eq_ignore_ascii_case("y")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set_model_subcommand() {
        let cli = Cli::try_parse_from(["git-auto-commit", "set-model", "llama3"]).expect("parse");
        match cli.command {
            Some(Commands::SetModel { model }) => assert_eq!(model, "llama3"),
            _ => panic!("expected SetModel command"),
        }
    }

    #[test]
    fn parses_set_format_subcommand() {
        let cli =
            Cli::try_parse_from(["git-auto-commit", "set-format", "type(scope): description"])
                .expect("parse");
        match cli.command {
            Some(Commands::SetFormat { format }) => {
                assert_eq!(format, "type(scope): description");
            }
            _ => panic!("expected SetFormat command"),
        }
    }

    #[test]
    fn parses_default_commit_command() {
        let cli = Cli::try_parse_from(["git-auto-commit"]).expect("parse");
        assert!(cli.command.is_none());
    }

    #[test]
    fn confirmation_is_case_insensitive() {
        assert!(confirms_commit("y"));
        assert!(confirms_commit("Y"));
        assert!(!confirms_commit("n"));
        assert!(!confirms_commit(""));
    }
}
