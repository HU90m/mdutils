use std::borrow::Cow;

use anyhow::{anyhow, Result};
use mdbook::book::{Book, BookItem};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use toml::value::{Table, Value};

use mdutils::{links::replace_links, markdown as md, regex::Regex};
use relative_path::PathExt;
use url::Url;

pub struct RegexReplace;

impl RegexReplace {
    pub fn new() -> RegexReplace {
        RegexReplace
    }

    fn get_replacements<'a>(
        &self,
        preproc_cfg: &'a Table,
        rep_type: &str,
    ) -> Result<Vec<(Regex, &'a str)>> {
        let mut replacements = Vec::new();
        let Some(val) = preproc_cfg.get(rep_type) else {
            return Ok(replacements);
        };

        let err_msg = || {
            Err(anyhow!(
                "'{}.{}' expects array of tables",
                self.name(),
                rep_type
            ))
        };
        let Value::Array(arr) = val else {
            return err_msg();
        };
        for val in arr {
            let Value::Table(tab) = val else {
                return err_msg();
            };
            let (Some(Value::String(pattern)), Some(Value::String(replacement))) =
                (tab.get("regex"), tab.get("replacement"))
            else {
                return err_msg();
            };
            replacements.push((Regex::new(pattern)?, replacement))
        }
        Ok(replacements)
    }
}

impl Preprocessor for RegexReplace {
    fn name(&self) -> &str {
        "replace"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let Some(preproc_cfg) = ctx.config.get_preprocessor(self.name()) else {
            return Ok(book);
        };
        let link_replacements = self.get_replacements(preproc_cfg, "link_replacements")?;
        let local_link_replacements =
            self.get_replacements(preproc_cfg, "local_link_replacements")?;

        let regex_replace = |book_item: &mut BookItem| {
            let BookItem::Chapter(chapter) = book_item else {
                return;
            };
            let chapter_path_opt = chapter.path.as_ref().map(|chapter_file| {
                let mut path = ctx.root.clone();
                path.push(chapter_file);
                path.pop();
                path
            });
            let replace_fn = |link: &str| {
                // If it's a local link, run through the local link replacements.
                let is_not_url = Url::parse(link).is_err();
                if let (Some(chapter_path), true) = (&chapter_path_opt, is_not_url) {
                    let absolute_path = {
                        let mut path = chapter_path.clone();
                        path.push(link);
                        path
                    };
                    let relative_path = absolute_path.relative_to(&ctx.root)?.normalize();

                    for (re, replacement) in &local_link_replacements {
                        if let Cow::Owned(new_link) =
                            re.replace(relative_path.as_str(), *replacement)
                        {
                            return Ok(Some(new_link));
                        }
                    }
                }
                // If no local link replacements have matched,
                // run through the link replacements.
                for (re, replacement) in &link_replacements {
                    if let Cow::Owned(new_link) = re.replace(link, *replacement) {
                        return Ok(Some(new_link));
                    }
                }
                Ok(None)
            };

            let content = &chapter.content;
            let ast = md::to_mdast(content, &Default::default()).unwrap();
            // It's safe to unwrap here, because we know `replace_fn` always returns Ok.
            if let Cow::Owned(new_content) = replace_links(content, &ast, replace_fn).unwrap() {
                chapter.content = new_content
            }
        };
        book.for_each_mut(regex_replace);

        Ok(book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn preprocessor_run() -> Result<()> {
        let input_json = r##"
        [
            {
                "root": "/path/to/book",
                "config": {
                    "book": {
                        "authors": ["AUTHOR"],
                        "language": "en",
                        "multilingual": false,
                        "src": "src",
                        "title": "TITLE"
                    },
                    "preprocessor": {
                        "replace": {
                            "link_replacements": [
                                { "regex": ".*", "replacement": "https://hugom.uk" }
                            ]
                        }
                    }
                },
                "renderer": "html",
                "mdbook_version": "0.4.21"
            },
            {
                "sections": [
                    {
                        "Chapter": {
                            "name": "Chapter 1",
                            "content": "[foo](bar.md) <https://bbc.co.uk>\n",
                            "number": [1],
                            "sub_items": [],
                            "path": "chapter_1.md",
                            "source_path": "chapter_1.md",
                            "parent_names": []
                        }
                    }
                ],
                "__non_exhaustive": null
            }
        ]"##;

        let (ctx, book) = mdbook::preprocess::CmdPreprocessor::parse_input(input_json.as_bytes())?;
        let mut expected = book.clone();
        expected.for_each_mut(|book_item| {
            let BookItem::Chapter(chapter) = book_item else {
                return;
            };
            chapter.content = "[foo](https://hugom.uk) <https://hugom.uk>\n".to_string();
        });

        let actual = RegexReplace::new().run(&ctx, book)?;

        assert_eq!(actual, expected);
        Ok(())
    }
}
