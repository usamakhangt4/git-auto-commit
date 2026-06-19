# git-auto-commit

Generate Git commit messages from staged changes using a local Ollama model.

## Features

- Reads staged changes and asks Ollama for a commit message
- Prompts for confirmation before running `git commit`
- Auto-selects an installed chat model when none is configured
- Configurable commit message format and Ollama model
- Skips common build artifacts, lockfiles, and binary files by default
- Summarizes very large diffs when the prompt would exceed 8,000 characters
- Detects bootstrap commits (many new files) and adjusts the prompt

## Prerequisites

- [Git](https://git-scm.com/)
- [Ollama](https://ollama.com/) running locally (default: `http://localhost:11434`)
- At least one chat model pulled, for example:

```bash
ollama pull llama3.2:3b
```

To build from source:

- Rust **1.75+** ([rustup](https://rustup.rs/))

## Installation

### Quick install (no Rust required)

macOS and Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/usamakhangt4/git-auto-commit/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/usamakhangt4/git-auto-commit/main/install.ps1 | iex
```

The installers download the latest precompiled binary from
[GitHub Releases](https://github.com/usamakhangt4/git-auto-commit/releases). Rust and
Cargo are not required. Ollama and Git are still required.

To choose another installation directory, set `GIT_AUTO_COMMIT_INSTALL_DIR`
before running the installer.

### Build from source

```bash
git clone https://github.com/usamakhangt4/git-auto-commit.git
cd git-auto-commit
cargo build --release
```

Copy the binary to a directory on your `PATH`:

```bash
cp target/release/git-auto-commit ~/.local/bin/
```

Ensure `~/.local/bin` is on your `PATH`.

### Install with Cargo

If you already have Rust installed:

```bash
cargo install --path .
```

## Usage

Stage changes, then run the tool inside a Git repository:

```bash
git add .
git-auto-commit
```

Example session:

```text
Using model: llama3.2:3b | Format: type(scope): description

Generated Message:
>>> feat(cli): add commit message generator

Commit? [y/N]: y
Successfully committed!
```

Answer `y` or `Y` to commit; anything else aborts.

### Subcommands

```bash
git-auto-commit set-model llama3.2:3b
git-auto-commit set-format "type(scope): description"
git-auto-commit --help
git-auto-commit --version
```

`set-model` and `set-format` work outside a Git repository.

## Configuration

Config files live in `~/.config/git-auto-commit/` (or the platform equivalent):

| File | Purpose |
|------|---------|
| `model.txt` | Ollama model name (e.g. `llama3.2:3b`). Empty or missing → auto-discovery |
| `format.txt` | Commit message format instruction for the model |
| `ignore.txt` | Path patterns to exclude from the prompt (one per line; `#` comments allowed) |

Precedence for format: `format.txt` → `COMMIT_FORMAT` env var → default `type(scope): description`.

Default ignore patterns include `target/`, `node_modules/`, lockfiles, minified assets, and common binary extensions. If `ignore.txt` is missing or empty, those defaults apply.

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OLLAMA_HOST` | `http://localhost:11434` | Ollama API base URL |
| `COMMIT_FORMAT` | `type(scope): description` | Format when `format.txt` is absent |
| `GIT_AUTO_COMMIT_TIMEOUT_SECS` | `300` | HTTP timeout for Ollama requests (seconds) |
| `GIT_AUTO_COMMIT_CONFIG_DIR` | (platform config dir) | Override config directory |

## Development

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

## License

MIT — see [LICENSE](LICENSE).
