use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "knead", about = "A zero-config static site generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build the site
    Build,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build => knead::build(),
    }
}
