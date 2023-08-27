mod preprocessor;

use std::{io, process};

use anyhow::Result;
use clap::{Arg, Command};
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use semver::{Version, VersionReq};

use preprocessor::RegexReplace;

pub fn cli() -> Command {
    Command::new("mdbook-regexreplace")
        .about("A mdbook preprocessor that parses the markdown and replaces regex matches")
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn main() -> Result<()> {
    let preprocessor = RegexReplace::new();

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

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<()> {
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
