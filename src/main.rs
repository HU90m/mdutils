mod md_utils;

use std::error::Error;
use std::{env, fs};

use markdown as md;

use md_utils::{get_links, replace_links};

fn main() -> Result<(), Box<dyn Error>> {
    let file = env::args().nth(1).unwrap();
    let content = fs::read_to_string(file)?;
    let ast = md::to_mdast(&content, &Default::default())?;
    let links = get_links(&ast, &content);
    println!("{links:?}");

    let replacements = [(regex::Regex::new(".{2}(.{2})")?, "$1 $0")];

    let new = replace_links(&content, &replacements);
    println!("{new}");
    Ok(())
}
