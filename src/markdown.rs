//! Markdown (CommonMark + GFM) ⇄ AST.
//!
//! Parser ([`from_markdown`](crate::Document::from_markdown)) and renderer
//! ([`to_markdown`](crate::Document::to_markdown), with per-node `to_markdown`
//! methods). Core
//! CommonMark + GFM (tables, strikethrough, task lists, autolinks) are
//! supported. Macros round-trip through Markdown via admonition alerts,
//! `<details>` (expand), and `<!--cf:…-->` markers (single for body-less
//! macros, paired for body-bearing ones); preserved Confluence markup
//! round-trips as `<!--cf-raw:…-->`.
//!
//! Confluence resource links/images (page/attachment/anchor) round-trip through
//! Markdown as a `confluence://` URI (see `LinkTarget::to_url`).

mod parse;
mod render;

#[cfg(test)]
mod tests;
