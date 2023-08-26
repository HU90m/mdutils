use core::ops::Range;

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
        let md_link = &content[md_range]; // str of markdown link
        // Try given pattens to extract url from markdown syntax
        let Some(caps) = regexes.into_iter().find_map(|re| re.captures(md_link)) else {
            panic!("Parser Error: '{md_link}' is not a valid markdown link.");
        };
        let m = caps
            .get(1)
            .expect("Expected regex group not present, check pattern.");
        // span of just the url
        md_range_start + m.start().. md_range_start + m.end()
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
