use tree_sitter::{Query, QueryCursor};
use tree_sitter_md::MarkdownParser;

/// Extracts the first atx heading at level 1 in the document
/// Returning the raw markdown of the title if found.
pub fn get_title(input: &str) -> Option<&str> {
    let tree = {
        let mut parser = MarkdownParser::default();
        parser.parse(input.as_bytes(), None).unwrap()
    };
    let block_query = Query::new(
        &tree_sitter_md::language(),
        "(atx_heading (atx_h1_marker) (inline) @title)",
    )
    .unwrap();

    QueryCursor::new()
        .matches(
            &block_query,
            tree.block_tree().root_node(),
            input.as_bytes(),
        )
        .next()
        .and_then(|matches| matches.captures.first())
        .map(|capture| capture.node)
        .map(|node| &input[node.byte_range()])
}

#[cfg(test)]
mod test {
    use super::*;
    use std::error::Error;

    #[test]
    fn replace_links_check() -> Result<(), Box<dyn Error>> {
        let input = "
## hello there

not atx style :(
----------------

not another one!
===========

## sanity returns
# why at the bottom?";
        let actual = get_title(&input);
        assert_eq!(actual, Some("why at the bottom?"));
        Ok(())
    }
}
