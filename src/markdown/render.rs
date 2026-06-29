//! Renderer: AST -> Markdown (CommonMark + GFM).

use std::fmt::{self, Display, Formatter};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

use super::parse::Marker;
use crate::ast::{Alignment, Block, Document, ImageSource, Inline, LinkTarget, Macro, MacroBody, Table, Task};

impl Document {
    /// Renders the document to Markdown (CommonMark + GFM).
    pub fn to_markdown(&self) -> String {
        Md(&self.blocks[..]).to_string()
    }
}

/// Renders an AST node to Markdown. A render-owned wrapper: every `Display`
/// impl targets `Md<…>`, never an `ast` type, so the shared AST stays closed.
/// Slices follow each kind's join convention — `[Inline]` concatenates,
/// `[Block]` joins with blank lines, and `[Vec<Inline>]` is a single table row.
struct Md<T>(T);

impl Display for Md<&[Block]> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (i, block) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n\n")?;
            }
            write!(f, "{}", Md(block))?;
        }
        Ok(())
    }
}

impl Display for Md<&[Inline]> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for inline in self.0 {
            write!(f, "{}", Md(inline))?;
        }
        Ok(())
    }
}

impl Display for Md<&Block> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Block::Heading { level, content } => write!(f, "{} {}", "#".repeat(*level as usize), Md(&content[..])),
            Block::Paragraph(content) => write!(f, "{}", Md(&content[..])),
            Block::CodeBlock { language, code } => {
                let lang = language.as_deref().unwrap_or("");
                write!(f, "```{lang}\n{code}\n```")
            },
            Block::BlockQuote(blocks) => {
                let quoted = Md(&blocks[..])
                    .to_string()
                    .lines()
                    .map(|line| if line.is_empty() { ">".to_string() } else { format!("> {line}") })
                    .collect::<Vec<_>>()
                    .join("\n");
                f.write_str(&quoted)
            },
            Block::List { ordered, items } => {
                let mut out = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let marker = if *ordered { format!("{}. ", i + 1) } else { "- ".to_string() };
                    let indent = " ".repeat(marker.len());
                    let mut item_str = String::new();
                    for (j, block) in item.iter().enumerate() {
                        let rendered = Md(block).to_string();
                        if j == 0 {
                            item_str.push_str(&marker);
                            item_str.push_str(&rendered.indent(&indent, false));
                        } else {
                            item_str.push('\n');
                            item_str.push_str(&rendered.indent(&indent, true));
                        }
                    }
                    out.push(item_str);
                }
                f.write_str(&out.join("\n"))
            },
            Block::ThematicBreak => f.write_str("---"),
            Block::Table(t) => write!(f, "{}", Md(t)),
            Block::TaskList(tasks) => {
                for (i, task) in tasks.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{}", Md(task))?;
                }
                Ok(())
            },
            Block::Macro(m) => write!(f, "{}", Md(m)),
            Block::RawConfluence(s) => f.write_str(&Marker::raw(s)),
        }
    }
}

impl Display for Md<&Inline> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Inline::Text(t) => f.write_str(&t.escape_markdown()),
            Inline::Strong(c) => write!(f, "**{}**", Md(&c[..])),
            Inline::Emphasis(c) => write!(f, "*{}*", Md(&c[..])),
            Inline::Strikethrough(c) => write!(f, "~~{}~~", Md(&c[..])),
            Inline::Code(c) => write!(f, "`{c}`"),
            Inline::Link { target, title, content } => {
                let url = Md(target).url();
                match title {
                    Some(t) => write!(f, "[{}]({url} \"{t}\")", Md(&content[..])),
                    None => write!(f, "[{}]({url})", Md(&content[..])),
                }
            },
            Inline::Image { source, alt } => write!(f, "![{alt}]({})", Md(source).url()),
            Inline::SoftBreak => f.write_str("\n"),
            Inline::HardBreak => f.write_str("  \n"),
            Inline::Macro(m) => write!(f, "{}", Md(m)),
            Inline::RawConfluence(s) => f.write_str(&Marker::raw(s)),
        }
    }
}

impl Md<&Macro> {
    fn marker(&self) -> String {
        let mut out = format!("<!--cf:{}", self.0.name);
        for (k, v) in &self.0.params {
            out.push_str(&format!(" {k}=\"{}\"", v.replace('"', "&quot;").replace("-->", "--&gt;")));
        }
        out.push_str("-->");
        out
    }

    fn body(&self) -> String {
        match &self.0.body {
            MacroBody::RichText(blocks) => Md(&blocks[..]).to_string(),
            MacroBody::PlainText(text) => text.clone(),
            MacroBody::Empty => String::new(),
        }
    }

    /// The GFM alert token for an admonition macro (`note` → `NOTE`, …), or
    /// `None` if the macro is not an admonition.
    fn alert_token(&self) -> Option<&'static str> {
        match self.0.name.as_str() {
            "note" => Some("NOTE"),
            "tip" => Some("TIP"),
            "warning" => Some("WARNING"),
            "info" => Some("IMPORTANT"),
            _ => None,
        }
    }

    fn alert(&self, token: &str) -> String {
        let mut out = format!("> [!{token}]");
        for line in self.body().lines() {
            out.push('\n');
            if line.is_empty() {
                out.push('>');
            } else {
                out.push_str("> ");
                out.push_str(line);
            }
        }
        out
    }

    fn details(&self) -> String {
        let title = self.0.param("title").unwrap_or_default();
        format!("<details><summary>{title}</summary>\n\n{}\n\n</details>", self.body())
    }
}

impl Display for Md<&Macro> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Untitled admonitions render as GFM alerts; expand renders as
        // `<details>`; body-less macros (status, toc, unknown) use single
        // markers; rich-body macros (panel, titled admonitions, unknown) use
        // paired markers.
        let m = self.0;
        let untitled = m.param("title").is_none();
        if let Some(token) = self.alert_token()
            && untitled
        {
            return f.write_str(&self.alert(token));
        }
        if m.name == "expand" {
            return f.write_str(&self.details());
        }
        if matches!(m.body, MacroBody::Empty) {
            return f.write_str(&self.marker());
        }
        write!(f, "{}\n\n{}\n\n<!--/cf:{}-->", self.marker(), self.body(), m.name)
    }
}

impl Display for Md<&Task> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mark = if self.0.checked { "x" } else { " " };
        write!(f, "- [{mark}] {}", Md(&self.0.content[..]))
    }
}

impl Md<&Table> {
    fn delim(&self) -> String {
        let cells: Vec<&str> = (0..self.0.head.len())
            .map(|i| match self.0.align.get(i).copied().unwrap_or(Alignment::None) {
                Alignment::None => "---",
                Alignment::Left => ":--",
                Alignment::Center => ":-:",
                Alignment::Right => "--:",
            })
            .collect();
        format!("| {} |", cells.join(" | "))
    }
}

impl Display for Md<&Table> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{}", Md(&self.0.head[..]), self.delim())?;
        for row in &self.0.rows {
            write!(f, "\n{}", Md(&row[..]))?;
        }
        Ok(())
    }
}

impl Display for Md<&[Vec<Inline>]> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let cells: Vec<String> = self
            .0
            .iter()
            .map(|c| Md(&c[..]).to_string().replace('|', "\\|"))
            .collect();
        write!(f, "| {} |", cells.join(" | "))
    }
}

impl Md<&LinkTarget> {
    fn url(&self) -> String {
        match self.0 {
            LinkTarget::External(url) => url.clone(),
            LinkTarget::Page { space, title } => {
                let space = space
                    .as_ref()
                    .map(|s| format!("space={}&", s.encode_uri()))
                    .unwrap_or_default();
                format!("confluence://page?{space}title={}", title.encode_uri())
            },
            LinkTarget::Content(id) => format!("confluence://content?id={}", id.encode_uri()),
            LinkTarget::Attachment(file) => format!("confluence://attachment?file={}", file.encode_uri()),
            LinkTarget::Anchor(name) => format!("confluence://anchor?name={}", name.encode_uri()),
        }
    }
}

impl Md<&ImageSource> {
    fn url(&self) -> String {
        match self.0 {
            ImageSource::External(url) => url.clone(),
            ImageSource::Attachment(file) => format!("confluence://attachment?file={}", file.encode_uri()),
        }
    }
}

/// Markdown render string transforms, as extension methods on `str`.
trait StrExt {
    /// Backslash-escapes the inline-significant CommonMark/GFM characters so
    /// literal text round-trips: emphasis (`*`/`_`), code (`` ` ``), links
    /// (`[`/`]`), strikethrough (`~`), and autolink/raw-HTML (`<`).
    /// Line-start-only markers (`#`, `>`, list bullets) are left alone to keep
    /// prose readable.
    fn escape_markdown(&self) -> String;

    /// Percent-encodes a `confluence://` URI component (RFC 3986 unreserved
    /// set: everything except `A-Za-z0-9` and `- _ . ~`).
    fn encode_uri(&self) -> String;

    /// Prefixes each non-empty line with `indent`. The first line is indented
    /// only when `indent_first` is set — a list item's opening line instead
    /// carries its bullet/number marker.
    fn indent(&self, indent: &str, indent_first: bool) -> String;
}

impl StrExt for str {
    fn escape_markdown(&self) -> String {
        let mut out = String::with_capacity(self.len());
        for ch in self.chars() {
            if matches!(ch, '\\' | '`' | '*' | '_' | '[' | ']' | '~' | '<') {
                out.push('\\');
            }
            out.push(ch);
        }
        out
    }

    fn encode_uri(&self) -> String {
        const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
            .remove(b'-')
            .remove(b'_')
            .remove(b'.')
            .remove(b'~');

        utf8_percent_encode(self, UNRESERVED).to_string()
    }

    fn indent(&self, indent: &str, indent_first: bool) -> String {
        self.lines()
            .enumerate()
            .map(|(i, line)| {
                if line.is_empty() || (i == 0 && !indent_first) {
                    line.to_string()
                } else {
                    format!("{indent}{line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
