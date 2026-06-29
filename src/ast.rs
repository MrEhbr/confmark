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
/// others are Confluence resource references (`ri:page` / `ri:content-entity` /
/// `ri:attachment` / anchor) that round-trip through Markdown as a
/// `confluence://` URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    External(String),
    Page {
        space: Option<String>,
        title: String,
    },
    /// A page referenced by numeric content id (`ri:content-id`).
    Content(String),
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

/// A node visited during a [`Document`] traversal.
enum Node<'a> {
    Block(&'a Block),
    Inline(&'a Inline),
}

impl<'a> Node<'a> {
    fn block(self) -> Option<&'a Block> {
        match self {
            Node::Block(block) => Some(block),
            Node::Inline(_) => None,
        }
    }

    fn inline(self) -> Option<&'a Inline> {
        match self {
            Node::Inline(inline) => Some(inline),
            Node::Block(_) => None,
        }
    }
}

impl Document {
    /// Iterates over every [`Block`] in the document in depth-first pre-order,
    /// including blocks nested in list items, block quotes, and macro bodies.
    ///
    /// ```
    /// use confmark::{Document, ast::Block};
    ///
    /// let doc = Document::from_markdown("# Title\n\n## Section\n\ntext");
    /// let levels: Vec<u8> = doc
    ///     .blocks()
    ///     .filter_map(|block| match block {
    ///         Block::Heading { level, .. } => Some(*level),
    ///         _ => None,
    ///     })
    ///     .collect();
    /// assert_eq!(levels, [1, 2]);
    /// ```
    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.walk().filter_map(Node::block)
    }

    /// Iterates over every [`Inline`] in the document in depth-first pre-order,
    /// descending through nested inlines and every block that carries inline
    /// content (headings, paragraphs, task lists, table cells, macro bodies).
    ///
    /// ```
    /// use confmark::{
    ///     Document,
    ///     ast::{Inline, LinkTarget},
    /// };
    ///
    /// let doc = Document::from_markdown("see [a](https://a.test) and [b](https://b.test)");
    /// let urls: Vec<&str> = doc
    ///     .inlines()
    ///     .filter_map(|inline| match inline {
    ///         Inline::Link {
    ///             target: LinkTarget::External(url),
    ///             ..
    ///         } => Some(url.as_str()),
    ///         _ => None,
    ///     })
    ///     .collect();
    /// assert_eq!(urls, ["https://a.test", "https://b.test"]);
    /// ```
    pub fn inlines(&self) -> impl Iterator<Item = &Inline> {
        self.walk().filter_map(Node::inline)
    }

    fn walk(&self) -> std::vec::IntoIter<Node<'_>> {
        let mut out = Vec::new();
        for block in &self.blocks {
            block.walk(&mut out);
        }
        out.into_iter()
    }
}

impl Block {
    fn walk<'a>(&'a self, out: &mut Vec<Node<'a>>) {
        out.push(Node::Block(self));
        match self {
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                for inline in content {
                    inline.walk(out);
                }
            },
            Block::List { items, .. } => {
                for block in items.iter().flatten() {
                    block.walk(out);
                }
            },
            Block::TaskList(tasks) => {
                for inline in tasks.iter().flat_map(|task| &task.content) {
                    inline.walk(out);
                }
            },
            Block::BlockQuote(blocks) => {
                for block in blocks {
                    block.walk(out);
                }
            },
            Block::Table(table) => table.walk(out),
            Block::Macro(mac) => mac.walk(out),
            Block::CodeBlock { .. } | Block::ThematicBreak | Block::RawConfluence(_) => {},
        }
    }
}

impl Inline {
    fn walk<'a>(&'a self, out: &mut Vec<Node<'a>>) {
        out.push(Node::Inline(self));
        match self {
            Inline::Strong(content) | Inline::Emphasis(content) | Inline::Strikethrough(content) | Inline::Link { content, .. } => {
                for inline in content {
                    inline.walk(out);
                }
            },
            Inline::Macro(mac) => mac.walk(out),
            Inline::Text(_) | Inline::Code(_) | Inline::Image { .. } | Inline::SoftBreak | Inline::HardBreak | Inline::RawConfluence(_) => {},
        }
    }
}

impl Table {
    fn walk<'a>(&'a self, out: &mut Vec<Node<'a>>) {
        for cell in self.head.iter().chain(self.rows.iter().flatten()) {
            for inline in cell {
                inline.walk(out);
            }
        }
    }
}

impl Macro {
    fn walk<'a>(&'a self, out: &mut Vec<Node<'a>>) {
        if let MacroBody::RichText(blocks) = &self.body {
            for block in blocks {
                block.walk(out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn text(s: &str) -> Inline {
        Inline::Text(s.to_string())
    }

    fn para(content: Vec<Inline>) -> Block {
        Block::Paragraph(content)
    }

    fn link(url: &str, label: &str) -> Inline {
        Inline::Link {
            target: LinkTarget::External(url.to_string()),
            title: None,
            content: vec![text(label)],
        }
    }

    fn panel(blocks: Vec<Block>) -> Block {
        Block::Macro(Macro {
            name: "panel".to_string(),
            params: vec![],
            body: MacroBody::RichText(blocks),
        })
    }

    fn texts(doc: &Document) -> Vec<&str> {
        doc.inlines()
            .filter_map(|inline| match inline {
                Inline::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    fn external_urls(doc: &Document) -> Vec<&str> {
        doc.inlines()
            .filter_map(|inline| match inline {
                Inline::Link {
                    target: LinkTarget::External(url),
                    ..
                } => Some(url.as_str()),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::heading(Block::Heading { level: 1, content: vec![text("a"), link("https://x.test", "b")] }, vec!["a", "b"])]
    #[case::block_quote(Block::BlockQuote(vec![para(vec![text("q")])]), vec!["q"])]
    #[case::list(Block::List { ordered: false, items: vec![vec![para(vec![text("i1")]), para(vec![text("i2")])]] }, vec!["i1", "i2"])]
    #[case::task_list(Block::TaskList(vec![Task { checked: false, content: vec![text("t")] }]), vec!["t"])]
    #[case::table(
        Block::Table(Table { align: vec![Alignment::None], head: vec![vec![text("h")]], rows: vec![vec![vec![text("c")]]] }),
        vec!["h", "c"]
    )]
    #[case::macro_body(panel(vec![para(vec![text("inner")])]), vec!["inner"])]
    #[case::nested_inline(para(vec![Inline::Strong(vec![text("s")])]), vec!["s"])]
    fn inlines_visit_text_in_every_container(#[case] block: Block, #[case] expected: Vec<&str>) {
        assert_eq!(texts(&Document { blocks: vec![block] }), expected);
    }

    #[rstest]
    #[case::leaf(para(vec![text("x")]), 1)]
    #[case::block_quote(Block::BlockQuote(vec![para(vec![text("q")])]), 2)]
    #[case::list(Block::List { ordered: false, items: vec![vec![para(vec![text("a")]), para(vec![text("b")])]] }, 3)]
    #[case::macro_body(panel(vec![para(vec![text("inner")])]), 2)]
    fn blocks_count_includes_nested(#[case] block: Block, #[case] expected: usize) {
        assert_eq!(Document { blocks: vec![block] }.blocks().count(), expected);
    }

    #[rstest]
    #[case::top_level(vec![para(vec![link("https://a.test", "a")])], vec!["https://a.test"])]
    #[case::nested_in_macro(vec![panel(vec![para(vec![link("https://nested.test", "n")])])], vec!["https://nested.test"])]
    #[case::document_order(
        vec![para(vec![link("https://1.test", "a")]), panel(vec![para(vec![link("https://2.test", "b")])])],
        vec!["https://1.test", "https://2.test"]
    )]
    fn inlines_collect_external_links(#[case] blocks: Vec<Block>, #[case] expected: Vec<&str>) {
        assert_eq!(external_urls(&Document { blocks }), expected);
    }
}
