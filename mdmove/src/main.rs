use std::fs::{self, ReadDir};
use std::path::{Path, PathBuf};
use std::borrow::Cow;

use anyhow::{anyhow, Result};
use clap::Parser;
use pathdiff::diff_paths;

use mdutil_lib::{markdown as md, links::replace_links};

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
    let sources = paths;

    if sources.len() > 1 && !destination.is_dir() {
        return Err(anyhow!("Target {destination:?} not a directory"));
    }

    for source in sources {
        if source.is_dir() {
            move_dir(source.read_dir()?, &destination)?;
            fs::remove_dir(&source)?;
        }
        move_file(&source, &destination)?;
    }

    Ok(())
}

fn move_dir(sources: ReadDir, destination: &Path) -> Result<()> {
    for entry in sources {
        let source = entry?.path();
        if source.is_dir() {
            move_dir(source.read_dir()?, destination)?;
            fs::remove_dir(&source)?;
        }
        move_file(&source, destination)?;
    }
    Ok(())
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_absolute()
        || matches!(
            source.extension().and_then(|ext| ext.to_str()),
            Some("md" | "markdown"),
        )
    {
        return move_mdfile(source, destination);
    }
    let dest = destination_file(source, destination);
    fs::rename(source, dest)?;
    Ok(())
}

fn move_mdfile(source: &Path, destination: &Path) -> Result<()> {
    let content = fs::read_to_string(source)?;
    let ast = md::to_mdast(&content, &Default::default()).unwrap();

    let replacement = |link: &str| {
        let path = Path::new(link);
        path.metadata()?;
        if !path.is_absolute() {
            return Ok(None);
        }
        let abs_path = source.join(path);
        let new_link = diff_paths(abs_path, destination)
            .unwrap()
            .to_string_lossy()
            .into();
        Ok(Some(new_link))
    };
    match replace_links(&content, &ast, replacement)? {
        Cow::Owned(new_content) => {
            fs::write(destination_file(source, destination), new_content)?;
            fs::remove_file(source)?;
        },
        Cow::Borrowed(_) => {
            fs::rename(source, destination_file(source, destination))?;
        }
    }
    Ok(())
}

fn destination_file<'a>(source: &Path, destination: &'a Path) -> Cow<'a, Path> {
    if destination.is_dir() {
        // Can safely unwrap because source should be a file.
        let name = source.file_name().unwrap();
        Cow::Owned(destination.join(name))
    } else {
        Cow::Borrowed(destination)
    }
}
