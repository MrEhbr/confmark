# confmark

[![CI](https://github.com/MrEhbr/confmark/actions/workflows/checks.yml/badge.svg)](https://github.com/MrEhbr/confmark/actions)
[![crates.io](https://img.shields.io/crates/v/confmark.svg)](https://crates.io/crates/confmark)
[![docs.rs](https://img.shields.io/docsrs/confmark)](https://docs.rs/confmark)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024+-orange)](https://www.rust-lang.org)

Bidirectional converter between **Markdown** (CommonMark + GFM) and **Confluence
Storage Format** (XHTML), as a Rust library and a CLI.

Both directions parse into a single shared document AST that renders out to
either format, so `md → cf → md` and `cf → md → cf` preserve every supported
construct — and anything unmappable is preserved verbatim rather than dropped.

## Install

**Prebuilt binary** (Linux / macOS, x86_64 + arm64):

```bash
curl -fsSL https://raw.githubusercontent.com/MrEhbr/confmark/main/install.sh | sh
```

Installs to `/usr/local/bin` if writable, otherwise `~/.local/bin`. Override with
`--bin-dir <dir>` or pin a release with `--version <tag>`:

```bash
curl -fsSL https://raw.githubusercontent.com/MrEhbr/confmark/main/install.sh | sh -s -- --version v0.1.0 --bin-dir ~/.local/bin
```

**With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall)** (fetches the same prebuilt binary):

```bash
cargo binstall confmark
```

**From source:**

```bash
cargo install --path . # from a checkout
# or
just install
```

## CLI

`confmark` is a Unix filter: it reads stdin and writes stdout by default, and
also accepts file arguments.

```bash
# Markdown -> Confluence (stdin -> stdout)
echo '# Hello' | confmark --from md --to cf
# <h1>Hello</h1>

# Confluence -> Markdown, short flags
confmark -f cf -t md page.xml
# reads page.xml, writes Markdown to stdout

# File in, file out
confmark -f md -t cf notes.md -o notes.xml

# `-` is an explicit stdin/stdout sentinel
cat notes.md | confmark -f md -t cf -
```

## Library

```rust
use confmark::Document;

let xml = Document::from_markdown("# Title").to_confluence();
assert_eq!(xml, "<h1>Title</h1>");

let md = Document::from_confluence("<h1>Title</h1>").to_markdown();
assert_eq!(md, "# Title");
```

`Document` is the entry point: `from_markdown` / `from_confluence` parse,
`to_markdown` / `to_confluence` render.

## Supported constructs

| Group | Coverage |
|---|---|
| Blocks | headings, paragraphs, code blocks, lists, blockquotes, thematic breaks |
| Inlines | strong, emphasis, strikethrough, inline code, links, images, line breaks |
| GFM | tables (alignment md→cf only), strikethrough, task lists, autolinks |
| Links | external + Confluence resource links (page / attachment / anchor) via a reversible `confluence://` URI |
| Macros | `code` (↔ fenced block), admonitions `info`/`note`/`tip`/`warning` (↔ GFM alerts), `expand` (↔ `<details>`), `status`/`toc`/`panel` and any unknown macro (↔ `<!--cf:…-->` markers) |
| Preservation | unrecognized storage elements survive as `RawConfluence` (`<!--cf-raw:…-->` in Markdown) |

The full Markdown ↔ AST ↔ Confluence contract is in
[`docs/MAPPING.md`](docs/MAPPING.md), backed by the round-trip fixtures in
`tests/fixtures/`.

**Known lossy points:** per-column table alignment is dropped on md→cf (storage
format has no standard for it); admonition styling beyond type is not
expressible as a GFM alert.

## Development

```bash
just build # build
just test  # cargo nextest
just lint  # clippy + rustfmt --check
just fmt   # format
```

The toolchain is split: `rust-toolchain.toml` pins **stable** for build/test/clippy;
`rustfmt` runs on **nightly** (the config uses nightly-only options).

## Scope

Confluence target is **Storage Format** (the REST interchange XHTML) only — not
Wiki Markup or ADF, and no live REST API integration. Markdown is CommonMark +
GFM.

## License

Licensed under MIT. See [LICENSE](LICENSE) for details.
