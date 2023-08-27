use core::ops::Range;
use std::borrow::Cow;

use markdown::{mdast::Node, unist::Position};
use once_cell::sync::Lazy;
use regex::Regex;

const fn pos_to_range(pos: &Position) -> Range<usize> {
    pos.start.offset..pos.end.offset
}

/// Extracts links from an abstract syntax tree.
pub fn get_links(node: &Node, content: &str) -> Vec<Range<usize>> {
    /// <https://spec.commonmark.org/0.30/#inline-link>
    static INLINE_LINK: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?s)^\[.*\]\((?:\s*<)?(.*)(?:>\s*)?\)$").unwrap());
    /// <https://spec.commonmark.org/0.30/#autolink>
    static AUTOLINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)^<(.*)>$").unwrap());
    /// <https://spec.commonmark.org/0.30/#link-label>
    static LINK_LABEL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)^\[.*\]:\s*(.*)$").unwrap());

    let link_from_node = |pos, regexes: &[&Lazy<Regex>]| {
        let md_range = pos_to_range(pos); // span of markdown link
        let md_range_start = md_range.start;
        // str of markdown link
        let md_link = &content[md_range];
        // Try given pattens to extract url from markdown syntax
        let Some(caps) = regexes.iter().find_map(|re| re.captures(md_link)) else {
            panic!("Parser Error: '{md_link}' is not a valid markdown link.");
        };
        let m = caps
            .get(1)
            .expect("Expected regex group not present, check pattern.");
        // span of just the url
        md_range_start + m.start()..md_range_start + m.end()
    };

    let mut links = match node {
        Node::Link(link) => {
            let pos = link
                .position
                .as_ref()
                .expect("Node doesn't have a position.");
            vec![link_from_node(pos, &[&INLINE_LINK, &AUTOLINK])]
        }
        Node::Definition(link) => {
            let pos = link
                .position
                .as_ref()
                .expect("Node doesn't have a position.");
            vec![link_from_node(pos, &[&LINK_LABEL])]
        }
        _ => Vec::new(),
    };

    if let Some(children) = node.children() {
        for node in children {
            links.extend(get_links(node, content));
        }
    };
    links
}

pub fn replace_links<'a>(
    content: &'a str,
    ast: &Node,
    replacements: &[(Regex, &str)],
) -> Cow<'a, str> {
    let mut state: Option<(String, usize)> = None;
    for link in get_links(ast, content) {
        for (re, replacement) in replacements {
            let link_str = content[link.clone()].trim();
            // If there is a match, replace the link in a new string.
            if let Cow::Owned(new_link) = re.replace(link_str, *replacement) {
                let (new_content, cursor) = state.take().unwrap_or((String::new(), 0));
                state = Some((
                    new_content + &content[cursor..link.start] + &new_link,
                    link.end,
                ));
                break;
            }
        }
    }
    if let Some((mut new_content, idx)) = state {
        new_content += &content[idx..];
        Cow::Owned(new_content)
    } else {
        Cow::Borrowed(content)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use markdown as md;
    use std::error::Error;

    #[test]
    fn replace_links_check() -> Result<(), Box<dyn Error>> {
        let input = "[foo](bar.md) <https://bbc.co.uk>\n\n[bar]: ./foo.md\n";
        let expected = "[foo](https://hugom.uk) <https://hugom.uk>\n\n[bar]: https://hugom.uk\n";

        let ast = md::to_mdast(input, &Default::default()).unwrap();
        let replacements = [(Regex::new(".*")?, "https://hugom.uk")];
        let actual = replace_links(input, &ast, &replacements);

        assert_eq!(actual, expected);

        Ok(())
    }
}
