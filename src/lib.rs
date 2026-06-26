//! Bidirectional conversion between Markdown (CommonMark + GFM) and Confluence
//! Storage Format (XHTML), built around a shared document AST.
//!
//! [`Document`] is the entry point: parse with [`Document::from_markdown`] /
//! [`Document::from_confluence`], render with [`Document::to_markdown`] /
//! [`Document::to_confluence`]. Each format's parse + render live in its module
//! ([`markdown`], [`confluence`]); the shared node types live in [`ast`].
//!
//! The AST is also traversable via [`Document::blocks`] / [`Document::inlines`]
//! for extracting data (links, headings, …) from a parsed document.
//!
//! # Examples
//!
//! ```
//! use confmark::Document;
//!
//! // Markdown -> Confluence Storage Format.
//! let xml = Document::from_markdown("# Title").to_confluence();
//! assert_eq!(xml, "<h1>Title</h1>");
//!
//! // Confluence Storage Format -> Markdown.
//! let md = Document::from_confluence("<h1>Title</h1>").to_markdown();
//! assert_eq!(md, "# Title");
//! ```
#![forbid(unsafe_code)]

pub mod ast;
pub mod confluence;
pub mod markdown;

pub use ast::Document;
