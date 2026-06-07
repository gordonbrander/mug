use anyhow::Result;
use clap::{Parser, Subcommand};
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "italic", about = "A zero-config static site generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build the site
    Build {
        /// Include draft documents (`draft: true`) in the output
        #[arg(long)]
        drafts: bool,
    },
    /// Watch source dirs and rebuild on change
    Watch,
    /// Serve the built site locally with live reload
    Serve {
        /// Port to bind
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// Host to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
    },
    /// Scaffold a starter site at the given path. The path must not already exist.
    New {
        /// Output directory for the scaffolded site
        path: PathBuf,
    },
    /// Copy the configured theme's starter content into this project's content dir
    Scaffold,
    /// Remove the output directory
    Clean,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { drafts } => italic::build(drafts),
        Command::Watch => italic::watch(),
        Command::Serve { port, host } => italic::serve(host, port),
        Command::New { path } => italic::new(&path),
        Command::Scaffold => italic::scaffold(),
        Command::Clean => italic::clean(),
    }
}
