//! Confluence Storage Format (XHTML) ⇄ AST.
//!
//! Parser ([`from_confluence`](crate::Document::from_confluence)) and renderer
//! ([`to_confluence`](crate::Document::to_confluence), with per-node
//! `to_confluence` methods). Core
//! CommonMark + GFM (tables, strikethrough, task lists),
//! `<ac:structured-macro>` macros, and unrecognized markup (preserved verbatim
//! as `RawConfluence`) all round-trip.

mod diagram;
mod parse;
mod render;

#[cfg(test)]
mod tests;
