//! The shared document AST.
//!
//! [`Document`] is the canonical representation both sides parse into and
//! render out of. This module is pure data; each format's parser and renderer
//! live in its own module ([`crate::markdown`], [`crate::confluence`]). The
//! Markdown↔AST↔Confluence contract is documented in `docs/MAPPING.md`.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    TaskList(Vec<Task>),
    BlockQuote(Vec<Block>),
    Table(Table),
    ThematicBreak,
    Macro(Macro),
    /// Confluence storage markup with no AST mapping, preserved verbatim.
    RawConfluence(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Link {
        target: LinkTarget,
        title: Option<String>,
        content: Vec<Inline>,
    },
    Image {
        source: ImageSource,
        alt: String,
    },
    SoftBreak,
    HardBreak,
    Macro(Macro),
    /// Inline Confluence storage markup with no AST mapping, preserved
    /// verbatim.
    RawConfluence(String),
}

/// The destination of a [`Inline::Link`]. External links carry a URL; the
/// others are Confluence resource references (`ri:page` / `ri:attachment` /
/// anchor) that round-trip through Markdown as a `confluence://` URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    External(String),
    Page { space: Option<String>, title: String },
    Attachment(String),
    Anchor(String),
}

/// The source of an [`Inline::Image`]: an external URL (`ri:url`) or a
/// Confluence attachment (`ri:attachment`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    External(String),
    Attachment(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub checked: bool,
    pub content: Vec<Inline>,
}

/// A GFM/Confluence table. `head` is the header row; `rows` are body rows; each
/// cell is a sequence of inlines. `align` is per column (lossy toward
/// Confluence, which has no standard per-column alignment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub align: Vec<Alignment>,
    pub head: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// A Confluence `<ac:structured-macro>`. The same representation covers the
/// built-in macros (code, info, note, warning, tip, panel, expand, toc, status)
/// and any unknown macro, which is preserved rather than dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Macro {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub body: MacroBody,
}

/// The body of a [`Macro`]: empty (toc/status), raw text (code, via CDATA), or
/// nested storage markup (admonitions, panel, expand).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroBody {
    Empty,
    PlainText(String),
    RichText(Vec<Block>),
}

impl Macro {
    /// The value of the first parameter named `key`, if present.
    pub(crate) fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}
