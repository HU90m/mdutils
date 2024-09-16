use anyhow::{bail, Result};
use clap::Parser;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, fs};

use mdutils::headings::get_title;

const SUMMARY_MD: &str = "SUMMARY.md";

#[derive(Parser)]
struct Options {
    #[arg(id = "directory")]
    dir: Option<PathBuf>,
    /// Update the SUMMARY.md, if it is out of date.
    #[arg(short, long)]
    update: bool,
}

#[allow(unused)]
#[derive(Debug)]
struct Node {
    title: String,
    path: Option<PathBuf>,
    sub_nodes: Vec<Node>,
}
impl Node {
    fn from_dir(dir: &Path, default_title: String) -> Result<Option<Self>> {
        let mut title = default_title;
        let mut index_path = None;
        let mut sub_nodes = Vec::new();
        for entry_res in fs::read_dir(dir)? {
            let entry = entry_res?;
            let fs_name = entry.file_name();
            if fs_name == "README.md" || fs_name == "index.md" {
                if index_path.is_some() {
                    bail!("Two indexes present");
                }
                let path = entry.path();
                title = title_from_md_file(&path)?;
                index_path = Some(path);
            } else if let Some(node) = Self::from_entry(&entry)? {
                sub_nodes.push(node);
            }
        }
        if sub_nodes.is_empty() && index_path.is_none() {
            // Ignore directory if it doesn't contain any markdown files.
            Ok(None)
        } else {
            Ok(Some(Node {
                title,
                path: index_path,
                sub_nodes,
            }))
        }
    }

    fn from_entry(entry: &fs::DirEntry) -> Result<Option<Node>> {
        let fs_name = entry.file_name();
        let path = entry.path();
        let path_real = resolve_links(&path)?;
        let node = if path_real.is_dir() {
            let fs_name = fs_name.to_string_lossy().to_string();
            return Self::from_dir(&path_real, fs_name);
        } else if path.extension().is_some_and(|ext| ext == "md") && fs_name != "SUMMARY.md" {
            Self {
                title: title_from_md_file(&path_real)?,
                path: Some(path),
                sub_nodes: Vec::new(),
            }
        } else {
            return Ok(None);
        };
        Ok(Some(node))
    }

    fn sort(&mut self) {
        for sub_node in &mut self.sub_nodes {
            sub_node.sort()
        }
        self.sub_nodes.sort_by(|a, b| a.title.cmp(&b.title));
    }

    fn render_to_md(&self, depth: usize, out: &mut String) {
        let path = self
            .path
            .as_ref()
            .map(|p| p.to_string_lossy())
            .map(|p| p.to_string())
            .unwrap_or_default();

        out.extend(std::iter::repeat("  ").take(depth));
        *out += &format!("- [{}]({})\n", self.title, path);

        for node in &self.sub_nodes {
            node.render_to_md(depth + 1, out);
        }
    }
}

#[derive(Debug)]
struct Summary(Vec<Node>);
impl Summary {
    fn from_dir(dir: &Path) -> Result<Self> {
        let mut nodes = Vec::new();
        for entry_res in fs::read_dir(dir)? {
            if let Some(node) = Node::from_entry(&entry_res?)? {
                nodes.push(node);
            }
        }
        Ok(Self(nodes))
    }

    fn sort(mut self) -> Self {
        for node in &mut self.0 {
            node.sort()
        }
        self.0.sort_by(|a, b| a.title.cmp(&b.title));
        self
    }

    fn render_to_md(&self) -> String {
        let mut out = "# Summary\n\n".to_string();
        for node in &self.0 {
            node.render_to_md(0, &mut out);
        }
        out
    }
}

fn title_from_md_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path)?;
    if let Some(title) = get_title(&content) {
        Ok(title.to_string())
    } else {
        let Some(name) = path.file_stem().and_then(OsStr::to_str) else {
            bail!("Can't generate a title from this path: {}", path.display())
        };
        Ok(name.to_string())
    }
}

fn resolve_links(path: &Path) -> Result<Cow<'_, Path>> {
    if path.is_symlink() {
        let mut path = path.to_path_buf();
        let mut is_link = true;
        while is_link {
            path = fs::read_link(&path)?;
            is_link = path.is_symlink();
        }
        Ok(Cow::Owned(path))
    } else {
        Ok(Cow::Borrowed(path))
    }
}

fn main() -> Result<()> {
    let opts = Options::parse();
    let mut dir = match opts.dir {
        Some(dir) if dir.is_dir() => dir,
        Some(file) => bail!("{} is not a directory.", file.display()),
        None => env::current_dir()?,
    };
    env::set_current_dir(&dir)?;
    let new_summary = Summary::from_dir(&PathBuf::from("."))?
        .sort()
        .render_to_md();

    dir.push(SUMMARY_MD);
    if opts.update {
        println!("Writing summary to {}", dir.display());
        fs::write(SUMMARY_MD, new_summary).map_err(Into::into)
    } else {
        let Ok(current_summary) = fs::read_to_string(SUMMARY_MD) else {
            bail!("Couldn't find or open {}", dir.display());
        };
        if new_summary != current_summary {
            let diff = prettydiff::text::diff_lines(&current_summary, &new_summary);
            bail!("{} is out of date\n{diff}", dir.display());
        }
        Ok(())
    }
}
