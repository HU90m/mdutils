use markdown::mdast::{Node, Heading};
use super::pos_to_range;

/// Extracts the title from an abstract syntax tree.
pub fn get_title<'a>(node: &Node, content: &'a str) -> Option<&'a str> {
    if let Some(children) = node.children() {
        for node in children {
            if let Node::Heading(Heading {
                depth: 1,
                position: Some(pos),
                ..
            }) = node {
                let range = pos_to_range(pos);
                let title = &content[range]
                    .trim_start_matches('#')
                    .trim();
                return Some(title)
            }
        }
    };
    Default::default()
}
