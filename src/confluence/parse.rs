//! Parser: Confluence Storage Format (XHTML) -> AST.

use quick_xml::{
    Reader, XmlVersion,
    escape::resolve_predefined_entity,
    events::{BytesStart, Event},
};

use super::diagram::DiagramMacro;
use crate::ast::{Block, Document, ImageSource, Inline, LinkTarget, Macro, MacroBody, Table, Task};

impl Document {
    /// Parses a Confluence Storage Format (XHTML) fragment into a [`Document`].
    /// Core CommonMark + GFM, external/resource links, and any
    /// `<ac:structured-macro>` (captured generically as a `Macro` node) are
    /// supported; unrecognized elements are preserved verbatim as
    /// `RawConfluence` (sliced from the source by byte span).
    pub fn from_confluence(src: &str) -> Document {
        let mut reader = Reader::from_str(src);
        let mut builder = Builder::new();
        loop {
            let start = reader.buffer_position() as usize;
            match reader.read_event() {
                Ok(Event::Start(e)) if is_transparent(e.name().as_ref()) => {},
                Ok(Event::End(e)) if is_transparent(e.name().as_ref()) => {},
                Ok(Event::Start(e)) => match Frame::from_start(&e) {
                    Some(frame) => builder.stack.push(frame),
                    None => {
                        if reader.read_to_end(e.name()).is_ok() {
                            builder.raw(src[start..reader.buffer_position() as usize].to_string());
                        }
                    },
                },
                Ok(Event::Empty(e)) => {
                    if !builder.empty(&e) && Frame::from_start(&e).is_none() && !is_transparent(e.name().as_ref()) {
                        builder.raw(src[start..reader.buffer_position() as usize].to_string());
                    }
                },
                Ok(Event::End(_)) => builder.end(),
                Ok(Event::Text(e)) => builder.text(&e.decode().unwrap_or_default()),
                Ok(Event::GeneralRef(r)) => match r.resolve_char_ref() {
                    Ok(Some(ch)) => builder.text(ch.encode_utf8(&mut [0u8; 4])),
                    _ => {
                        let name = r.decode().unwrap_or_default();
                        match resolve_predefined_entity(&name).or_else(|| resolve_entity(&name)) {
                            Some(s) => builder.text(s),
                            None => builder.text(&format!("&{name};")),
                        }
                    },
                },
                Ok(Event::CData(e)) => builder.cdata(String::from_utf8_lossy(&e.into_inner()).into_owned()),
                Ok(Event::Eof) => break,
                Ok(_) => {},
                Err(_) => break,
            }
        }
        builder.finish()
    }
}

/// An in-progress node while folding quick-xml events into the AST.
enum Frame {
    Doc(BlockContainer),
    Quote(BlockContainer),
    Item(BlockContainer),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Para(Vec<Inline>),
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strike(Vec<Inline>),
    Span(Vec<Inline>),
    Code(String),
    ExtLink {
        href: String,
        title: Option<String>,
        content: Vec<Inline>,
    },
    AcLink {
        target: Option<LinkTarget>,
        content: Vec<Inline>,
    },
    LinkBody(Vec<Inline>),
    AcImage {
        source: Option<ImageSource>,
        alt: String,
    },
    CodeMacro {
        language: Option<String>,
        code: String,
    },
    Param {
        name: String,
        value: String,
    },
    PlainBody(String),
    Macro {
        name: String,
        params: Vec<(String, String)>,
        body: MacroBody,
    },
    RichBody(BlockContainer),
    TaskList(Vec<Task>),
    Task {
        checked: bool,
        content: Vec<Inline>,
    },
    TaskStatus(String),
    TaskBody(Vec<Inline>),
    Table {
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Row {
        is_head: bool,
        cells: Vec<Vec<Inline>>,
    },
    Cell {
        header: bool,
        content: Vec<Inline>,
    },
}

/// Block-level children plus a run of trailing inlines not yet wrapped in a
/// paragraph. Owns the rule that loose inlines become a `Paragraph` when a
/// block is appended or the container is finalized.
#[derive(Default)]
struct BlockContainer {
    blocks: Vec<Block>,
    pending: Vec<Inline>,
}

impl BlockContainer {
    fn push_inline(&mut self, inline: Inline) {
        self.pending.push(inline);
    }

    fn push_block(&mut self, block: Block) {
        self.flush();
        self.blocks.push(block);
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.blocks
                .push(Block::Paragraph(std::mem::take(&mut self.pending)));
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush();
        self.blocks
    }
}

impl Frame {
    fn heading(level: u8) -> Self {
        Frame::Heading { level, content: Vec::new() }
    }

    /// Frames that collect block-level children plus a run of pending inlines.
    fn block_container_mut(&mut self) -> Option<&mut BlockContainer> {
        match self {
            Frame::Doc(c) | Frame::Quote(c) | Frame::Item(c) | Frame::RichBody(c) => Some(c),
            _ => None,
        }
    }

    /// Frames that collect inline children directly.
    /// Must cover the same variants as [`Frame::is_inline_container`].
    fn inline_sink_mut(&mut self) -> Option<&mut Vec<Inline>> {
        match self {
            Frame::Para(v)
            | Frame::Heading { content: v, .. }
            | Frame::Strong(v)
            | Frame::Emphasis(v)
            | Frame::Strike(v)
            | Frame::Span(v)
            | Frame::ExtLink { content: v, .. }
            | Frame::AcLink { content: v, .. }
            | Frame::LinkBody(v)
            | Frame::Cell { content: v, .. }
            | Frame::TaskBody(v) => Some(v),
            _ => None,
        }
    }

    /// Must cover the same variants as [`Frame::inline_sink_mut`].
    fn is_inline_container(&self) -> bool {
        matches!(
            self,
            Frame::Para(_)
                | Frame::Heading { .. }
                | Frame::Strong(_)
                | Frame::Emphasis(_)
                | Frame::Strike(_)
                | Frame::Span(_)
                | Frame::ExtLink { .. }
                | Frame::AcLink { .. }
                | Frame::LinkBody(_)
                | Frame::Cell { .. }
                | Frame::TaskBody(_)
        )
    }

    /// Maps a start/empty element name to the frame it opens, or `None` if the
    /// element is unrecognized (the caller preserves it verbatim as
    /// `RawConfluence`).
    fn from_start(e: &BytesStart) -> Option<Frame> {
        let frame = match e.name().as_ref() {
            b"p" => Frame::Para(Vec::new()),
            b"h1" => Frame::heading(1),
            b"h2" => Frame::heading(2),
            b"h3" => Frame::heading(3),
            b"h4" => Frame::heading(4),
            b"h5" => Frame::heading(5),
            b"h6" => Frame::heading(6),
            b"strong" => Frame::Strong(Vec::new()),
            b"em" => Frame::Emphasis(Vec::new()),
            b"span" => match attr(e, b"style") {
                Some(style) if style.contains("line-through") => Frame::Strike(Vec::new()),
                _ => Frame::Span(Vec::new()),
            },
            b"code" => Frame::Code(String::new()),
            b"blockquote" => Frame::Quote(BlockContainer::default()),
            b"ul" => Frame::List {
                ordered: false,
                items: Vec::new(),
            },
            b"ol" => Frame::List {
                ordered: true,
                items: Vec::new(),
            },
            b"li" => Frame::Item(BlockContainer::default()),
            b"a" => Frame::ExtLink {
                href: attr(e, b"href").unwrap_or_default(),
                title: attr(e, b"title"),
                content: Vec::new(),
            },
            b"ac:link" => Frame::AcLink {
                target: attr(e, b"ac:anchor").map(LinkTarget::Anchor),
                content: Vec::new(),
            },
            b"ac:link-body" | b"ac:plain-text-link-body" => Frame::LinkBody(Vec::new()),
            b"ac:image" => Frame::AcImage {
                source: None,
                alt: attr(e, b"ac:alt").unwrap_or_default(),
            },
            b"ac:structured-macro" => match attr(e, b"ac:name").as_deref() {
                Some("code") => Frame::CodeMacro {
                    language: None,
                    code: String::new(),
                },
                // A diagram macro carries its source in a CDATA body, like
                // `code`; the language is fixed by the macro name, not a param.
                name => match name.and_then(DiagramMacro::for_macro_name) {
                    Some(d) => Frame::CodeMacro {
                        language: Some(d.language.to_string()),
                        code: String::new(),
                    },
                    None => Frame::Macro {
                        name: name.unwrap_or_default().to_string(),
                        params: Vec::new(),
                        body: MacroBody::Empty,
                    },
                },
            },
            b"ac:parameter" => Frame::Param {
                name: attr(e, b"ac:name").unwrap_or_default(),
                value: String::new(),
            },
            b"ac:plain-text-body" => Frame::PlainBody(String::new()),
            b"ac:rich-text-body" => Frame::RichBody(BlockContainer::default()),
            b"ac:task-list" => Frame::TaskList(Vec::new()),
            b"ac:task" => Frame::Task {
                checked: false,
                content: Vec::new(),
            },
            b"ac:task-status" => Frame::TaskStatus(String::new()),
            b"ac:task-body" => Frame::TaskBody(Vec::new()),
            b"table" => Frame::Table {
                head: Vec::new(),
                rows: Vec::new(),
            },
            b"tr" => Frame::Row {
                is_head: false,
                cells: Vec::new(),
            },
            b"th" => Frame::Cell {
                header: true,
                content: Vec::new(),
            },
            b"td" => Frame::Cell {
                header: false,
                content: Vec::new(),
            },
            _ => return None,
        };
        Some(frame)
    }
}

struct Builder {
    stack: Vec<Frame>,
}

impl Builder {
    fn new() -> Self {
        Self {
            stack: vec![Frame::Doc(BlockContainer::default())],
        }
    }

    fn finish(mut self) -> Document {
        match self.stack.pop() {
            Some(Frame::Doc(c)) => Document { blocks: c.finish() },
            _ => Document::default(),
        }
    }

    fn empty(&mut self, e: &BytesStart) -> bool {
        match e.name().as_ref() {
            b"hr" => self.push_block(Block::ThematicBreak),
            b"br" => self.push_inline(Inline::HardBreak),
            b"ri:page" => {
                let target = match (attr(e, b"ri:content-title"), attr(e, b"ri:content-id")) {
                    (None, Some(id)) => LinkTarget::Content(id),
                    (title, _) => LinkTarget::Page {
                        space: attr(e, b"ri:space-key"),
                        title: title.unwrap_or_default(),
                    },
                };
                self.set_link_target(target);
            },
            b"ri:content-entity" => {
                let id = attr(e, b"ri:content-id").unwrap_or_default();
                self.set_link_target(LinkTarget::Content(id));
            },
            b"ri:attachment" => {
                let file = attr(e, b"ri:filename").unwrap_or_default();
                self.set_attachment(file);
            },
            b"ri:url" => {
                let url = attr(e, b"ri:value").unwrap_or_default();
                if let Some(Frame::AcImage { source, .. }) = self.stack.last_mut() {
                    *source = Some(ImageSource::External(url));
                }
            },
            _ => return false,
        }
        true
    }

    fn end(&mut self) {
        let Some(frame) = self.stack.pop() else { return };
        match frame {
            Frame::Para(content) => self.push_block(Block::Paragraph(content)),
            Frame::Heading { level, content } => self.push_block(Block::Heading { level, content }),
            Frame::Quote(c) => self.push_block(Block::BlockQuote(c.finish())),
            Frame::List { ordered, items } => self.push_block(Block::List { ordered, items }),
            Frame::Item(c) => {
                let blocks = c.finish();
                if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                    items.push(blocks);
                }
            },
            Frame::Strong(c) => self.push_inline(Inline::Strong(c)),
            Frame::Emphasis(c) => self.push_inline(Inline::Emphasis(c)),
            Frame::Strike(c) => self.push_inline(Inline::Strikethrough(c)),
            Frame::Span(content) => {
                for inline in content {
                    self.push_inline(inline);
                }
            },
            Frame::Code(s) => self.push_inline(Inline::Code(s)),
            Frame::ExtLink { href, title, content } => self.push_inline(Inline::Link {
                target: LinkTarget::External(href),
                title,
                content,
            }),
            Frame::AcLink { target, content } => {
                let target = target.unwrap_or_else(|| LinkTarget::External(String::new()));
                self.push_inline(Inline::Link { target, title: None, content });
            },
            Frame::LinkBody(inlines) => {
                if let Some(Frame::AcLink { content, .. }) = self.stack.last_mut() {
                    content.extend(inlines);
                }
            },
            Frame::AcImage { source, alt } => {
                let source = source.unwrap_or_else(|| ImageSource::External(String::new()));
                self.push_inline(Inline::Image { source, alt });
            },
            Frame::CodeMacro { language, code } => self.push_block(Block::CodeBlock { language, code }),
            Frame::Param { name, value } => match self.stack.last_mut() {
                Some(Frame::CodeMacro { language, .. }) if name == "language" => *language = Some(value),
                Some(Frame::Macro { params, .. }) => params.push((name, value)),
                _ => {},
            },
            Frame::PlainBody(text) => match self.stack.last_mut() {
                Some(Frame::CodeMacro { code, .. }) => *code = text,
                Some(Frame::Macro { body, .. }) => *body = MacroBody::PlainText(text),
                _ => {},
            },
            Frame::RichBody(c) => {
                let blocks = c.finish();
                if let Some(Frame::Macro { body, .. }) = self.stack.last_mut() {
                    *body = MacroBody::RichText(blocks);
                }
            },
            Frame::Macro { name, params, body } => {
                let m = Macro { name, params, body };
                if self.top_is_inline() {
                    self.push_inline(Inline::Macro(m));
                } else {
                    self.push_block(Block::Macro(m));
                }
            },
            Frame::Cell { header, content } => {
                if let Some(Frame::Row { is_head, cells }) = self.stack.last_mut() {
                    *is_head |= header;
                    cells.push(content);
                }
            },
            Frame::Row { is_head, cells } => {
                if let Some(Frame::Table { head, rows }) = self.stack.last_mut() {
                    if is_head {
                        *head = cells;
                    } else {
                        rows.push(cells);
                    }
                }
            },
            Frame::Table { head, rows } => self.push_block(Block::Table(Table { align: Vec::new(), head, rows })),
            Frame::TaskStatus(status) => {
                if let Some(Frame::Task { checked, .. }) = self.stack.last_mut() {
                    *checked = status.trim() == "complete";
                }
            },
            Frame::TaskBody(inlines) => {
                if let Some(Frame::Task { content, .. }) = self.stack.last_mut() {
                    *content = inlines;
                }
            },
            Frame::Task { checked, content } => {
                if let Some(Frame::TaskList(tasks)) = self.stack.last_mut() {
                    tasks.push(Task { checked, content });
                }
            },
            Frame::TaskList(tasks) => self.push_block(Block::TaskList(tasks)),
            Frame::Doc(_) => {},
        }
    }

    fn text(&mut self, t: &str) {
        match self.stack.last_mut() {
            Some(Frame::Code(s)) => s.push_str(t),
            Some(Frame::PlainBody(s)) => s.push_str(t),
            Some(Frame::TaskStatus(s)) => s.push_str(t),
            Some(Frame::Param { value, .. }) => value.push_str(t),
            _ => {
                if t.trim().is_empty() && !self.top_is_inline() {
                    return;
                }
                self.push_inline(Inline::Text(t.to_string()));
            },
        }
    }

    fn raw(&mut self, markup: String) {
        if self.top_is_inline() {
            self.push_inline(Inline::RawConfluence(markup));
        } else {
            self.push_block(Block::RawConfluence(markup));
        }
    }

    fn cdata(&mut self, s: String) {
        match self.stack.last_mut() {
            Some(Frame::PlainBody(b)) => b.push_str(&s),
            Some(Frame::LinkBody(v)) => v.push(Inline::Text(s)),
            _ => {},
        }
    }

    fn set_link_target(&mut self, target: LinkTarget) {
        if let Some(Frame::AcLink { target: t, .. }) = self.stack.last_mut() {
            *t = Some(target);
        }
    }

    fn set_attachment(&mut self, file: String) {
        match self.stack.last_mut() {
            Some(Frame::AcLink { target, .. }) => *target = Some(LinkTarget::Attachment(file)),
            Some(Frame::AcImage { source, .. }) => *source = Some(ImageSource::Attachment(file)),
            _ => {},
        }
    }

    fn top_is_inline(&self) -> bool {
        self.stack.last().is_some_and(Frame::is_inline_container)
    }

    fn push_inline(&mut self, inline: Inline) {
        let Some(frame) = self.stack.last_mut() else { return };
        if let Some(v) = frame.inline_sink_mut() {
            v.push(inline);
        } else if let Some(c) = frame.block_container_mut() {
            c.push_inline(inline);
        }
    }

    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Frame::List { items, .. }) => items.push(vec![block]),
            Some(frame) => {
                if let Some(c) = frame.block_container_mut() {
                    c.push_block(block);
                }
            },
            None => {},
        }
    }
}

/// Wrapper elements with no AST node: their children attach to the enclosing
/// frame, so they are not pushed onto the stack.
fn is_transparent(name: &[u8]) -> bool {
    matches!(name, b"thead" | b"tbody")
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| {
            a.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned())
        })
}

fn resolve_entity(name: &str) -> Option<&'static str> {
    match name {
        "nbsp" => Some("\u{a0}"),
        "mdash" => Some("—"),
        "ndash" => Some("–"),
        "hellip" => Some("…"),
        "copy" => Some("©"),
        "reg" => Some("®"),
        "trade" => Some("™"),
        _ => None,
    }
}
