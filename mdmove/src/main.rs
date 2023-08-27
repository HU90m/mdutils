use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The paths to the root of the opentitan repo.
    #[arg(num_args=2..)]
    paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let Cli { mut paths } = Cli::parse();
    let destination = paths.pop().unwrap();

    if paths.len() > 1 && !destination.is_dir() {
        return Err(anyhow!("Target {destination:?} not a directory"));
    }

    Ok(())
}
