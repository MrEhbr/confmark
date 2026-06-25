#![forbid(unsafe_code)]

use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use confmark::Document;

/// Bidirectional Markdown ⇄ Confluence Storage Format converter.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Source format to convert from.
    #[arg(short, long, value_enum)]
    from: Format,

    /// Target format to convert to.
    #[arg(short, long, value_enum)]
    to: Format,

    /// Input file; reads standard input when omitted.
    input: Option<PathBuf>,

    /// Output file; writes standard output when omitted.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    #[value(alias = "md")]
    Markdown,
    #[value(alias = "cf")]
    Confluence,
}

impl Format {
    fn parse(self, src: &str) -> Document {
        match self {
            Format::Markdown => Document::from_markdown(src),
            Format::Confluence => Document::from_confluence(src),
        }
    }

    fn render(self, doc: &Document) -> String {
        match self {
            Format::Markdown => doc.to_markdown(),
            Format::Confluence => doc.to_confluence(),
        }
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    if args.from == args.to {
        bail!("source and target formats must differ");
    }

    let input = match &args.input {
        Some(path) => fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?,
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .context("failed to read stdin")?;
            buf
        },
    };

    let output = args.to.render(&args.from.parse(&input));

    match &args.output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))?;
            eprintln!("Saved {}", path.display());
            Ok(())
        },
        None => io::stdout()
            .write_all(output.as_bytes())
            .context("failed to write stdout"),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:?}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;
    use rstest::rstest;

    use super::Format;

    #[rstest]
    #[case("md", Format::Markdown)]
    #[case("markdown", Format::Markdown)]
    #[case("cf", Format::Confluence)]
    #[case("confluence", Format::Confluence)]
    fn parses_format_aliases(#[case] input: &str, #[case] expected: Format) {
        assert_eq!(Format::from_str(input, true).unwrap(), expected);
    }
}
