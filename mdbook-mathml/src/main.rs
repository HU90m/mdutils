use std::borrow::Cow;
use std::{io, process};

use anyhow::Result;
use clap::{Arg, Command};
use latex2mathml::{latex_to_mathml, DisplayStyle};
use mdbook::book::{Book, BookItem};
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use pulldown_cmark::{Event, Options, Parser};
use semver::{Version, VersionReq};

pub fn cli() -> Command {
    Command::new("mdbook-mathml")
        .about("A mdbook preprocessor that converts inline maths to mathml.")
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn main() -> Result<()> {
    let preprocessor = MathMlPreprocessor;

    let args = cli().get_matches();

    if let Some(sub_args) = args.subcommand_matches("supports") {
        let renderer = sub_args
            .get_one::<String>("renderer")
            .expect("Required argument");
        let supported = preprocessor.supports_renderer(renderer);
        process::exit(if supported { 0 } else { 1 });
    }
    handle_preprocessing(&preprocessor)
}

fn handle_preprocessing(pre: &impl Preprocessor) -> Result<()> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

pub struct MathMlPreprocessor;

impl Preprocessor for MathMlPreprocessor {
    fn name(&self) -> &str {
        "replace"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let regex_replace = |book_item: &mut BookItem| {
            let BookItem::Chapter(chapter) = book_item else {
                return;
            };
            if let Cow::Owned(new_content) = replace_latex(&chapter.content).unwrap() {
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

fn replace_latex(markdown: &str) -> Result<Cow<'_, str>> {
    let extensions = Options::ENABLE_GFM
        | Options::ENABLE_MATH
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let mut replacements = vec![];
    for (event, range) in Parser::new_ext(markdown, extensions).into_offset_iter() {
        let style = match event {
            Event::InlineMath(_) => DisplayStyle::Inline,
            Event::DisplayMath(_) => DisplayStyle::Block,
            _ => continue,
        };
        let snippet = markdown[range.clone()]
            .trim_start_matches('$')
            .trim_end_matches('$');
        let mathml = latex_to_mathml(snippet, style)?;
        replacements.push((range, mathml));
    }
    if replacements.is_empty() {
        return Ok(Cow::Borrowed(markdown));
    }

    let mut output_md = markdown.to_string();
    for (range, mathml) in replacements.iter().rev() {
        output_md = output_md[..range.start].to_string() + mathml + &output_md[range.end..];
    }
    return Ok(Cow::Owned(output_md));
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn convert_markdown() -> Result<()> {
        let input = r##"
# Hello

$a = b$

$$b = c$$

$$
c = d
$$a
        "##;

        let expected = r##"
# Hello

<math xmlns="http://www.w3.org/1998/Math/MathML" display="inline"><mi>a</mi><mo>=</mo><mi>b</mi></math>

<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><mi>b</mi><mo>=</mo><mi>c</mi></math>

<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><mi>c</mi><mo>=</mo><mi>d</mi></math>a
        "##;
        let output = replace_latex(input)?;
        assert!(expected == output);
        Ok(())
    }
}
