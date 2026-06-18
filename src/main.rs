use anyhow::Result;

fn main() -> Result<()> {
    if let Err(error) = git_auto_commit::run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
    Ok(())
}
