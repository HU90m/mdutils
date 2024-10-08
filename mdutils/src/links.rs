use core::ops::Range;
use std::borrow::Cow;

use anyhow::Result;
use regex::Regex;
use tree_sitter_md::MarkdownParser;

pub fn get_links(input: &str) -> Vec<Range<usize>> {
    let tree = {
        let mut parser = MarkdownParser::default();
        parser.parse(input.as_bytes(), None).unwrap()
    };
    let mut query_cur = tree_sitter::QueryCursor::new();

    // There are two different tree types needed to express a markdown document.
    // A top level 'block' tree and a number of inline trees.
    // We need a different query for each.
    let block_query =
        tree_sitter::Query::new(&tree_sitter_md::language(), "(link_destination) @link").unwrap();
    let inline_query = tree_sitter::Query::new(
        &tree_sitter_md::inline_language(),
        "[(link_destination) (uri_autolink)] @link",
    )
    .unwrap();

    // Find the matches in the block tree.
    let block_matches = query_cur.matches(
        &block_query,
        tree.block_tree().root_node(),
        input.as_bytes(),
    );
    // Find all the matches in the inline trees.
    let inline_matches = tree.inline_trees().iter().flat_map(|inline_tree| {
        query_cur.matches(&inline_query, inline_tree.root_node(), input.as_bytes())
    });
    // Convert the matches into the byte range of the link destination.
    block_matches
        .chain(inline_matches)
        .flat_map(|matches| matches.captures.iter())
        .map(|capture| capture.node)
        .map(|node| {
            // If it's an auto link, e.g. `<https://hugom.uk>`,
            // we need want to remove the angle brackets.
            if node.kind() == "uri_autolink" {
                let range = node.byte_range();
                (range.start + 1)..(range.end - 1)
            } else {
                node.byte_range()
            }
        })
        .collect()
}

/// Will only error if `replacement` returns an error.
pub fn replace_links(
    content: &str,
    replacement: impl Fn(&str) -> Result<Option<String>>,
) -> Result<Cow<'_, str>> {
    let mut state: Option<(String, usize)> = None;
    let mut links = get_links(content);
    links.sort_by_key(|range| range.start);
    for link in links {
        let link_str = content[link.clone()].trim();
        if let Some(new_link) = replacement(link_str)? {
            let (new_content, cursor) = state.take().unwrap_or((String::new(), 0));
            state = Some((
                new_content + &content[cursor..link.start] + &new_link,
                link.end,
            ));
        }
    }
    if let Some((mut new_content, idx)) = state {
        new_content += &content[idx..];
        Ok(Cow::Owned(new_content))
    } else {
        Ok(Cow::Borrowed(content))
    }
}

pub fn regexreplace_links<'a>(content: &'a str, replacements: &[(Regex, &str)]) -> Cow<'a, str> {
    let replacement_fn = move |link: &str| {
        for (re, replacement) in replacements {
            // If there is a match, replace the link in a new string.
            if let Cow::Owned(new_link) = re.replace(link, *replacement) {
                return Ok(Some(new_link));
            }
        }
        Ok(None)
    };
    // Replacement_fn can't error so, replace_links won't error.
    replace_links(content, replacement_fn).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::error::Error;

    #[test]
    fn replace_links_check() -> Result<(), Box<dyn Error>> {
        let input = "[foo](bar.md) <https://bbc.co.uk>\n\n[bar]: ./foo.md\n";
        let expected = "[foo](https://hugom.uk) <https://hugom.uk>\n\n[bar]: https://hugom.uk\n";

        let replacements = [(Regex::new(".*")?, "https://hugom.uk")];
        let actual = regexreplace_links(input, &replacements);

        assert_eq!(actual, expected);
        Ok(())
    }
}
