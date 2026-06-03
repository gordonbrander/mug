use anyhow::Result;
use clap::{Parser, Subcommand};
use mug::report::{ServeHandle, StderrReporter};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "mug", about = "A zero-config static site generator")]
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
        /// Project directory (defaults to the current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Watch source dirs and rebuild on change
    Watch {
        /// Project directory (defaults to the current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Serve the built site locally with live reload
    Serve {
        /// Port to bind
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// Host to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
        /// Project directory (defaults to the current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Scaffold a starter site at the given path. The path must not already exist.
    New {
        /// Output directory for the scaffolded site
        path: PathBuf,
    },
    /// Remove the output directory
    Clean {
        /// Project directory (defaults to the current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { drafts, path } => {
            let start = Instant::now();
            let report = mug::build(&path, drafts)?;
            eprintln!("built {} pages in {:?}", report.pages, start.elapsed());
            Ok(())
        }
        Command::Watch { path } => mug::watch(&path, Arc::new(StderrReporter)),
        Command::Serve { port, host, path } => {
            mug::serve(&path, host, port, Arc::new(StderrReporter), ServeHandle::new())
        }
        Command::New { path } => mug::new(&path),
        Command::Clean { path } => {
            if let Some(dir) = mug::clean(&path)? {
                eprintln!("cleaned {}", dir.display());
            }
            Ok(())
        }
    }
}
