//! Parser: Markdown (CommonMark + GFM) -> AST, walked from comrak's AST.

use comrak::{
    Arena, Options,
    nodes::{AlertType, AstNode, ListType, NodeValue, TableAlignment},
    parse_document,
};
use percent_encoding::percent_decode_str;

use crate::ast::{Alignment, Block, Document, ImageSource, Inline, LinkTarget, Macro, MacroBody, Table, Task};

impl Document {
    /// Parses Markdown (CommonMark + GFM) source into a [`Document`].
    pub fn from_markdown(src: &str) -> Document {
        let arena = Arena::new();
        let mut options = Options::default();
        options.extension.table = true;
        options.extension.strikethrough = true;
        options.extension.tasklist = true;
        options.extension.autolink = true;
        options.extension.alerts = true;
        let root = parse_document(&arena, src, &options);
        Document { blocks: root.blocks() }
    }
}

/// Walks comrak's AST into the neutral AST. Implemented on `&AstNode` so the
/// recursion reads as `node.blocks()` / `child.inlines()`.
trait NodeExt<'a> {
    /// Block-level children. Confluence markers and `<details>` arrive as
    /// sibling HTML blocks and are resolved against the blocks emitted so far.
    fn blocks(self) -> Vec<Block>;
    fn inlines(self) -> Vec<Inline>;
    fn list_block(self, list_type: ListType) -> Block;
    fn table_block(self, alignments: &[TableAlignment]) -> Block;
    /// Concatenated `Text` descendants (used for image alt text).
    fn text_content(self) -> String;
}

impl<'a> NodeExt<'a> for &'a AstNode<'a> {
    fn blocks(self) -> Vec<Block> {
        let mut out = Vec::new();
        for child in self.children() {
            match child.data.borrow().value.clone() {
                NodeValue::Paragraph => out.push(Block::Paragraph(child.inlines())),
                NodeValue::Heading(h) => out.push(Block::Heading {
                    level: h.level,
                    content: child.inlines(),
                }),
                NodeValue::ThematicBreak => out.push(Block::ThematicBreak),
                NodeValue::BlockQuote => out.push(Block::BlockQuote(child.blocks())),
                NodeValue::CodeBlock(cb) => {
                    let language = (cb.fenced && !cb.info.is_empty()).then(|| cb.info.split(' ').next().unwrap_or(&cb.info).to_string());
                    let code = cb
                        .literal
                        .strip_suffix('\n')
                        .unwrap_or(&cb.literal)
                        .to_string();
                    out.push(Block::CodeBlock { language, code });
                },
                NodeValue::List(list) => out.push(child.list_block(list.list_type)),
                NodeValue::Table(t) => out.push(child.table_block(&t.alignments)),
                NodeValue::Alert(a) => {
                    let name = match a.alert_type {
                        AlertType::Note => "note",
                        AlertType::Tip => "tip",
                        AlertType::Important => "info",
                        AlertType::Warning | AlertType::Caution => "warning",
                    };
                    let params = match a.title {
                        Some(t) if !t.is_empty() => vec![("title".to_string(), t)],
                        _ => Vec::new(),
                    };
                    out.push(Block::Macro(Macro {
                        name: name.to_string(),
                        params,
                        body: MacroBody::RichText(child.blocks()),
                    }));
                },
                NodeValue::HtmlBlock(hb) => match Marker::parse(&hb.literal) {
                    Marker::Open(m) => out.push(Block::Macro(m)),
                    // Retroactively wrap the blocks emitted since the matching open
                    // marker into that macro's `RichText` body.
                    Marker::Close(name) => {
                        let open = |b: &Block| matches!(b, Block::Macro(m) if m.name == name && matches!(m.body, MacroBody::Empty));
                        if let Some(idx) = out.iter().rposition(open) {
                            let body = out.drain(idx + 1..).collect();
                            if let Some(Block::Macro(m)) = out.get_mut(idx) {
                                m.body = MacroBody::RichText(body);
                            }
                        }
                    },
                    Marker::Raw(raw) | Marker::Other(raw) => out.push(Block::RawConfluence(raw)),
                },
                _ => {},
            }
        }
        out
    }

    fn inlines(self) -> Vec<Inline> {
        let mut out = Vec::new();
        for child in self.children() {
            match child.data.borrow().value.clone() {
                NodeValue::Text(t) => out.push(Inline::Text(t.into())),
                NodeValue::SoftBreak => out.push(Inline::SoftBreak),
                NodeValue::LineBreak => out.push(Inline::HardBreak),
                NodeValue::Code(c) => out.push(Inline::Code(c.literal)),
                NodeValue::Emph => out.push(Inline::Emphasis(child.inlines())),
                NodeValue::Strong => out.push(Inline::Strong(child.inlines())),
                NodeValue::Strikethrough => out.push(Inline::Strikethrough(child.inlines())),
                NodeValue::Link(link) => out.push(Inline::Link {
                    target: LinkTarget::from_url(&link.url),
                    title: (!link.title.is_empty()).then_some(link.title),
                    content: child.inlines(),
                }),
                NodeValue::Image(link) => out.push(Inline::Image {
                    source: ImageSource::from_url(&link.url),
                    alt: child.text_content(),
                }),
                NodeValue::HtmlInline(h) => out.push(Inline::from_html(&h)),
                _ => {},
            }
        }
        out
    }

    fn list_block(self, list_type: ListType) -> Block {
        let ordered = matches!(list_type, ListType::Ordered);
        let mut items: Vec<Vec<Block>> = Vec::new();
        let mut tasks: Vec<Option<bool>> = Vec::new();
        for item in self.children() {
            let task = match &item.data.borrow().value {
                NodeValue::TaskItem(t) => Some(t.symbol.is_some()),
                _ => None,
            };
            items.push(item.blocks());
            tasks.push(task);
        }
        if !items.is_empty() && tasks.iter().all(Option::is_some) {
            let task_items = items
                .into_iter()
                .zip(tasks)
                .map(|(blocks, checked)| Task {
                    checked: checked.unwrap_or(false),
                    content: match blocks.into_iter().next() {
                        Some(Block::Paragraph(inlines)) => inlines,
                        _ => Vec::new(),
                    },
                })
                .collect();
            Block::TaskList(task_items)
        } else {
            Block::List { ordered, items }
        }
    }

    fn table_block(self, alignments: &[TableAlignment]) -> Block {
        let align = alignments
            .iter()
            .map(|a| match a {
                TableAlignment::None => Alignment::None,
                TableAlignment::Left => Alignment::Left,
                TableAlignment::Center => Alignment::Center,
                TableAlignment::Right => Alignment::Right,
            })
            .collect();
        let mut head = Vec::new();
        let mut rows = Vec::new();
        for row in self.children() {
            let cells: Vec<Vec<Inline>> = row.children().map(|c| c.inlines()).collect();
            if matches!(row.data.borrow().value, NodeValue::TableRow(true)) {
                head = cells;
            } else {
                rows.push(cells);
            }
        }
        Block::Table(Table { align, head, rows })
    }

    fn text_content(self) -> String {
        let mut out = String::new();
        for child in self.children() {
            if let NodeValue::Text(t) = &child.data.borrow().value {
                out.push_str(t);
            }
        }
        out
    }
}

/// A Confluence directive carried in the Markdown as an HTML comment or a
/// `<details>` element.
pub(super) enum Marker {
    /// `<!--cf:NAME …-->` or `<details>` — opens an (initially body-less)
    /// macro.
    Open(Macro),
    /// `<!--/cf:NAME-->` or `</details>` — closes the matching open macro.
    Close(String),
    /// `<!--cf-raw:…-->` — preserved Confluence markup.
    Raw(String),
    /// Any other raw HTML, kept verbatim.
    Other(String),
}

impl Marker {
    fn parse(html: &str) -> Marker {
        let html = html.trim();
        let comment = |tag: &str| html.strip_prefix(tag).and_then(|s| s.strip_suffix("-->"));
        comment("<!--/cf:")
            .map(|name| Marker::Close(name.trim().to_string()))
            .or_else(|| comment("<!--cf-raw:").map(|inner| Marker::Raw(inner.replace("&#10;", "\n").replace("--&gt;", "-->"))))
            .or_else(|| Macro::from_marker(html).map(Marker::Open))
            .or_else(|| {
                html.starts_with("<details").then(|| {
                    let title = html
                        .split_once("<summary>")
                        .and_then(|(_, rest)| rest.split_once("</summary>"))
                        .map_or_else(String::new, |(title, _)| title.to_string());
                    let params: Vec<_> = (!title.is_empty())
                        .then(|| ("title".to_string(), title))
                        .into_iter()
                        .collect();
                    Marker::Open(Macro {
                        name: "expand".to_string(),
                        params,
                        body: MacroBody::Empty,
                    })
                })
            })
            .or_else(|| {
                html.starts_with("</details")
                    .then(|| Marker::Close("expand".to_string()))
            })
            .unwrap_or_else(|| Marker::Other(html.to_string()))
    }

    /// Serializes preserved Confluence markup as a `<!--cf-raw:…-->` comment so
    /// it survives a Markdown round-trip (`ac:`-namespaced tags are not valid
    /// CommonMark HTML). The inverse of [`parse`]'s [`Raw`] arm: newlines and
    /// `-->` are escaped to keep a single comment.
    ///
    /// [`parse`]: Marker::parse
    /// [`Raw`]: Marker::Raw
    pub(super) fn raw(markup: &str) -> String {
        let encoded = markup.replace("-->", "--&gt;").replace('\n', "&#10;");
        format!("<!--cf-raw:{encoded}-->")
    }
}

impl Inline {
    /// Interprets inline raw HTML as a macro, preserved markup, or verbatim
    /// HTML. Close markers and `<details>` are block-level only, so
    /// anything that isn't an open macro or `cf-raw` is kept exactly as
    /// written.
    fn from_html(html: &str) -> Inline {
        match Marker::parse(html) {
            Marker::Open(m) => Inline::Macro(m),
            Marker::Raw(raw) => Inline::RawConfluence(raw),
            _ => Inline::RawConfluence(html.to_string()),
        }
    }
}

impl LinkTarget {
    pub(super) fn from_url(url: &str) -> LinkTarget {
        let Some(rest) = url.strip_prefix("confluence://") else {
            return LinkTarget::External(url.to_string());
        };
        let (host, query) = rest.split_once('?').unwrap_or((rest, ""));
        let decode = |s: &str| percent_decode_str(s).decode_utf8_lossy().into_owned();
        let params: Vec<(String, String)> = query
            .split('&')
            .filter_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                Some((decode(k), decode(v)))
            })
            .collect();
        let get = |key: &str| {
            params
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        };
        match host {
            "page" => LinkTarget::Page {
                space: get("space"),
                title: get("title").unwrap_or_default(),
                content_id: get("id"),
            },
            "content" => LinkTarget::Content(get("id").unwrap_or_default()),
            "attachment" => LinkTarget::Attachment(get("file").unwrap_or_default()),
            "anchor" => LinkTarget::Anchor(get("name").unwrap_or_default()),
            _ => LinkTarget::External(url.to_string()),
        }
    }
}

impl ImageSource {
    pub(super) fn from_url(url: &str) -> ImageSource {
        match LinkTarget::from_url(url) {
            LinkTarget::Attachment(file) => ImageSource::Attachment(file),
            _ => ImageSource::External(url.to_string()),
        }
    }
}

impl Macro {
    /// Parses a `<!--cf:NAME k="v" …-->` marker into a body-less macro, or
    /// `None` if the comment is not a `cf:` marker.
    fn from_marker(html: &str) -> Option<Macro> {
        let inner = html
            .trim()
            .strip_prefix("<!--cf:")?
            .strip_suffix("-->")?
            .trim();
        let (name, mut rest) = match inner.split_once(char::is_whitespace) {
            Some((n, r)) => (n.to_string(), r.trim_start()),
            None => (inner.to_string(), ""),
        };
        let mut params = Vec::new();
        while !rest.is_empty() {
            let eq = rest.find('=')?;
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start().strip_prefix('"')?;
            let end = after.find('"')?;
            params.push((
                key,
                after[..end]
                    .replace("--&gt;", "-->")
                    .replace("&quot;", "\""),
            ));
            rest = after[end + 1..].trim_start();
        }
        Some(Macro {
            name,
            params,
            body: MacroBody::Empty,
        })
    }
}
