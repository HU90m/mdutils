use anyhow::{bail, Result};
use clap::Parser;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};

use mdutil_lib::headings::get_title;
use mdutil_lib::markdown as md;

#[derive(Parser)]
struct Options {
    #[arg(id = "directory")]
    dir: Option<PathBuf>,
}

#[allow(unused)]
#[derive(Debug)]
struct Node {
    title: String,
    path: Option<PathBuf>,
    sub_nodes: Option<Vec<Node>>,
}
impl Node {
    fn from_dir(dir: &Path, default_title: String) -> Result<Self> {
        let mut title = default_title;
        let mut index_path = None;
        let mut sub_nodes = None;
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
                sub_nodes
                    .get_or_insert_with(Vec::<Node>::default)
                    .push(node);
            }
        }
        Ok(Node {
            title,
            path: index_path,
            sub_nodes,
        })
    }

    fn from_entry(entry: &fs::DirEntry) -> Result<Option<Node>> {
        let fs_name = entry.file_name();
        let path = entry.path();
        let path_real = resolve_links(&path)?;
        let node = if path_real.is_dir() {
            let fs_name = fs_name.to_string_lossy().to_string();
            Self::from_dir(&path_real, fs_name)?
        } else if path.extension().is_some_and(|ext| ext == "md") && fs_name != "SUMMARY.md" {
            Self {
                title: title_from_md_file(&path_real)?,
                path: Some(path),
                sub_nodes: None,
            }
        } else {
            return Ok(None);
        };
        Ok(Some(node))
    }

    fn render_to_md(&self, depth: usize, out: &mut String) {
        let path = self
            .path
            .as_ref()
            .map(|p| p.to_string_lossy())
            .map(|p| p.to_string())
            .unwrap_or_else(String::new);

        out.extend(std::iter::repeat("  ").take(depth));
        *out += &format!("- [{}]({})\n", self.title, path);

        if let Some(sub_nodes) = &self.sub_nodes {
            for node in sub_nodes.iter() {
                node.render_to_md(depth + 1, out);
            }
            out.push('\n');
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
    let ast = md::to_mdast(&content, &Default::default()).unwrap();
    if let Some(title) = get_title(&ast, &content) {
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
    let new_summary = Summary::from_dir(&PathBuf::from("."))?.render_to_md();

    dir.push("SUMMARY.md");
    println!("Writing summary to {}", dir.display());
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open("SUMMARY.md")?
        .write_all(new_summary.as_bytes())
        .map_err(Into::into)
}
