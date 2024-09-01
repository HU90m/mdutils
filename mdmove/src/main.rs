use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::fs::{self, ReadDir};
use std::path::{
    Component::{self, Normal, RootDir},
    Path, PathBuf,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use pathdiff::diff_paths;

use mdutils::{links::replace_links, markdown as md};

#[derive(Debug, Default)]
struct MoveList(HashMap<PathBuf, PathBuf>);
impl MoveList {
    /// Expects the given path to be absolute.
    fn get_path_after_move(&self, path: &Path) -> Option<PathBuf> {
        for (from, to) in &self.0 {
            if path.starts_with(from) {
                // unwrap safe due to `starts_with` check above
                let mut new_path = to.join(path.strip_prefix(from).unwrap());
                new_path = normalize_path(&new_path);
                return Some(new_path);
            }
        }
        None
    }
}
impl FromIterator<(PathBuf, PathBuf)> for MoveList {
    fn from_iter<T: IntoIterator<Item = (PathBuf, PathBuf)>>(iter: T) -> MoveList {
        let mut list = Self::default();
        list.0.extend(iter);
        list
    }
}

type ChangeList = HashMap<PathBuf, String>;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The paths to be moved
    #[arg(num_args=2..)]
    paths: Vec<PathBuf>,
    /// The root of the notes.
    /// Defaults to the current directory.
    #[arg(short, long)]
    root: Option<PathBuf>,
    /// Print changes but don't actually perform moves
    #[arg(short, long)]
    dry_run: bool,
}

fn main() -> Result<()> {
    let Cli {
        mut paths,
        root,
        dry_run,
    } = Cli::parse();
    let mut destination = paths.pop().unwrap();
    if destination.is_relative() {
        destination = normalize_path(&env::current_dir()?.join(destination));
    }
    let sources = paths;
    let root = root
        .map(|r| r.canonicalize())
        .unwrap_or_else(env::current_dir)?;

    for source in &sources {
        if !source.exists() {
            return Err(anyhow!("{source:?} doesn't exist"));
        }
        if source.to_str().is_none() {
            return Err(anyhow!("{source:?} isn't valid utf8"));
        }
    }

    let moves = get_move_list(sources, destination)?;
    let changes = get_change_list(root.read_dir()?, &moves, &root)?;

    for (source, destination) in moves.0 {
        println!("moving {source:#?} to {destination:#?}");
        if !dry_run {
            fs::rename(source, destination)?;
        }
    }

    for (path, change) in changes {
        println!("writing changes to {path:#?}");
        if !dry_run {
            fs::write(path, change)?;
        }
    }
    Ok(())
}

fn get_move_list(mut sources: Vec<PathBuf>, destination: PathBuf) -> Result<MoveList> {
    if sources.len() == 1 {
        // ok to unwrap because the length is checked above
        let source = sources.pop().unwrap().canonicalize()?;
        let name = source
            .file_name()
            // ok to unwarp because canonicalized
            .unwrap();
        let dest = if destination.exists() {
            destination.join(name)
        } else {
            destination
        };
        return Ok(MoveList::from_iter([(source, dest)]));
    }
    if !destination.is_dir() {
        return Err(anyhow!("Target {destination:?} not a directory"));
    }
    let moves: MoveList = sources
        .into_iter()
        .map(|source| {
            let source = source
                .canonicalize()
                // ok to unwrap because path known to exist
                .unwrap();
            let name = source
                .file_name()
                // ok to unwarp because canonicalized
                .unwrap();
            let new_path = destination.join(name);
            (source, new_path)
        })
        .collect();
    Ok(moves)
}

fn get_change_list(dir: ReadDir, moves: &MoveList, root: &Path) -> Result<ChangeList> {
    let mut change_list = ChangeList::new();
    for entry in dir {
        let mut file = entry?.path();
        if file.is_symlink() {
            file = file.canonicalize()?;
        }
        if file.is_dir() {
            let list = get_change_list(file.read_dir()?, moves, root)?;
            change_list.extend(list);
        } else if file.is_file() {
            let list = change_file(&file, moves, root)?;
            change_list.extend(list);
        }
    }
    Ok(change_list)
}

fn change_file(file: &Path, moves: &MoveList, root: &Path) -> Result<ChangeList> {
    let mut change_list = ChangeList::new();
    if !matches!(
        file.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown"),
    ) {
        return Ok(change_list);
    }
    let file_dest = moves
        .get_path_after_move(file)
        .unwrap_or_else(|| file.to_path_buf());
    let file_dir = file.parent().unwrap();
    let file_dest_dir = file_dest.parent().unwrap();

    let content = fs::read_to_string(file)?;
    let ast = md::to_mdast(&content, &Default::default()).unwrap();

    let replacement = |link: &str| {
        // 1. make link absolute based on current file dir or root
        // 2. if link is to a file in the move list,
        //    change the link an absolute address of where the file will be
        //    after the moves
        // 3. make the link relative to the file containing it after the moves
        //      *(this may be the same as before the moves)*
        //      Unless the link was absolute,
        //      in which case make the link relative to the root
        let (link_path, frag) = match link.split_once('#') {
            Some((p, fragment)) => (p, Some(fragment)),
            None => (link, None),
        };
        if link_path.is_empty() {
            return Ok(None);
        }
        let link_path = Path::new(link_path);
        let mut comps = link_path.components();
        // get absolute path to linked file
        let (link_path_abs, was_abs) = match comps.next() {
            Some(Normal(str)) if str == "https:" || str == "http:" => return Ok(None),
            Some(RootDir) => (root.join(comps.as_path()), true),
            _ => (file_dir.join(link_path), false),
        };
        let mut link_path_abs = normalize_path(&link_path_abs);
        if !link_path_abs.exists() {
            println!(
                "warning: '{}' in '{}' doesn't exist",
                link_path_abs.display(),
                file.display(),
            );
            return Ok(None);
        }
        if let Some(link_path_post_move) = moves.get_path_after_move(&link_path_abs) {
            link_path_abs = link_path_post_move
        };

        let new_link_path = if was_abs {
            let path_rel = link_path_abs.strip_prefix(root).unwrap();
            Path::new("/").join(path_rel)
        } else {
            diff_paths(link_path_abs, file_dest_dir).unwrap()
        };
        let mut new_link = new_link_path.to_string_lossy().to_string();
        if let Some(fragment) = frag {
            new_link += "#";
            new_link += fragment;
        }
        Ok(Some(new_link))
    };
    if let Cow::Owned(new_content) = replace_links(&content, &ast, replacement)? {
        change_list.insert(file_dest, new_content);
    };
    Ok(change_list)
}

// From <https://github.com/rust-lang/cargo/blob/fede83ccf973457de319ba6fa0e36ead454d2e20/src/cargo/util/paths.rs#L61>
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}
