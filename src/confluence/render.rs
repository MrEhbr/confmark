//! Renderer: AST -> Confluence Storage Format (XHTML).

use std::fmt::{self, Display, Formatter};

use super::diagram::DiagramMacro;
use crate::ast::{Block, Document, ImageSource, Inline, LinkTarget, Macro, MacroBody, Table, Task};

impl Document {
    /// Renders the document to Confluence Storage Format (XHTML).
    pub fn to_confluence(&self) -> String {
        Cf(&self.blocks[..]).to_string()
    }
}

/// Renders an AST node to Confluence Storage Format. A render-owned wrapper:
/// every `Display` impl targets `Cf<…>`, never an `ast` type, so the shared AST
/// stays closed. Slices follow each kind's join convention — `[Inline]`
/// concatenates, `[Block]` joins with newlines.
struct Cf<T>(T);

impl Display for Cf<&[Block]> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (i, block) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{}", Cf(block))?;
        }
        Ok(())
    }
}

impl Display for Cf<&[Inline]> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for inline in self.0 {
            write!(f, "{}", Cf(inline))?;
        }
        Ok(())
    }
}

impl Cf<&[Block]> {
    /// Renders "tight" for a list item: a standalone block paragraph drops its
    /// `<p>` wrapper (Confluence emits `<li>text</li>`, not
    /// `<li><p>text</p></li>`).
    fn tight(&self) -> String {
        self.0
            .iter()
            .map(|block| match block {
                Block::Paragraph(content) => Cf(&content[..]).to_string(),
                other => Cf(other).to_string(),
            })
            .collect()
    }
}

impl Display for Cf<&Block> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Block::Heading { level, content } => write!(f, "<h{level}>{}</h{level}>", Cf(&content[..])),
            Block::Paragraph(content) => write!(f, "<p>{}</p>", Cf(&content[..])),
            Block::CodeBlock { language, code } => {
                if let Some(d) = language.as_deref().and_then(DiagramMacro::for_language) {
                    let params: String = d
                        .params
                        .iter()
                        .map(|(k, v)| {
                            format!(
                                "\n<ac:parameter ac:name=\"{}\">{}</ac:parameter>",
                                k.escape_attr(),
                                v.escape_text()
                            )
                        })
                        .collect();
                    return write!(
                        f,
                        "<ac:structured-macro ac:name=\"{}\" \
                         ac:schema-version=\"{}\">{params}\n<ac:plain-text-body><![CDATA[{code}]]></ac:plain-text-body>\n</ac:structured-macro>",
                        d.name, d.schema_version
                    );
                }
                let lang = match language {
                    Some(l) => format!("\n<ac:parameter ac:name=\"language\">{}</ac:parameter>", l.escape_text()),
                    None => String::new(),
                };
                write!(
                    f,
                    "<ac:structured-macro ac:name=\"code\">{lang}\n<ac:plain-text-body><![CDATA[{code}]]></ac:plain-text-body>\n</ac:structured-macro>"
                )
            },
            Block::BlockQuote(blocks) => write!(f, "<blockquote>\n{}\n</blockquote>", Cf(&blocks[..])),
            Block::List { ordered, items } => {
                let tag = if *ordered { "ol" } else { "ul" };
                write!(f, "<{tag}>")?;
                for item in items {
                    write!(f, "<li>{}</li>", Cf(&item[..]).tight())?;
                }
                write!(f, "</{tag}>")
            },
            Block::ThematicBreak => f.write_str("<hr/>"),
            Block::Table(t) => write!(f, "{}", Cf(t)),
            Block::TaskList(tasks) => {
                let body = tasks
                    .iter()
                    .map(|t| Cf(t).to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(f, "<ac:task-list>\n{body}\n</ac:task-list>")
            },
            Block::Macro(m) => write!(f, "{}", Cf(m)),
            Block::RawConfluence(s) => f.write_str(s),
        }
    }
}

impl Display for Cf<&Macro> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let m = self.0;
        let params: String = m
            .params
            .iter()
            .map(|(k, v)| {
                format!(
                    "<ac:parameter ac:name=\"{}\">{}</ac:parameter>",
                    k.escape_attr(),
                    v.escape_text()
                )
            })
            .collect();
        let body = match &m.body {
            MacroBody::Empty => String::new(),
            MacroBody::PlainText(text) => format!("<ac:plain-text-body><![CDATA[{text}]]></ac:plain-text-body>"),
            MacroBody::RichText(blocks) => format!("<ac:rich-text-body>{}</ac:rich-text-body>", Cf(&blocks[..])),
        };
        write!(
            f,
            "<ac:structured-macro ac:name=\"{}\">{params}{body}</ac:structured-macro>",
            m.name.escape_attr()
        )
    }
}

impl Display for Cf<&Task> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let status = if self.0.checked { "complete" } else { "incomplete" };
        write!(
            f,
            "<ac:task><ac:task-status>{status}</ac:task-status><ac:task-body>{}</ac:task-body></ac:task>",
            Cf(&self.0.content[..])
        )
    }
}

impl Cf<&Table> {
    fn row(cells: &[Vec<Inline>], is_head: bool) -> String {
        let tag = if is_head { "th" } else { "td" };
        let body: String = cells
            .iter()
            .map(|c| format!("<{tag}>{}</{tag}>", Cf(&c[..])))
            .collect();
        format!("<tr>{body}</tr>")
    }
}

impl Display for Cf<&Table> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut rows = vec![Self::row(&self.0.head, true)];
        rows.extend(self.0.rows.iter().map(|r| Self::row(r, false)));
        write!(f, "<table>\n<tbody>\n{}\n</tbody>\n</table>", rows.join("\n"))
    }
}

impl Display for Cf<&Inline> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0 {
            Inline::Text(t) => f.write_str(&t.escape_text()),
            Inline::Strong(c) => write!(f, "<strong>{}</strong>", Cf(&c[..])),
            Inline::Emphasis(c) => write!(f, "<em>{}</em>", Cf(&c[..])),
            Inline::Strikethrough(c) => write!(f, "<span style=\"text-decoration: line-through;\">{}</span>", Cf(&c[..])),
            Inline::Code(c) => write!(f, "<code>{}</code>", c.escape_text()),
            Inline::Link { target, title, content } => {
                let body = Cf(&content[..]).to_string();
                f.write_str(&Cf(target).render(title.as_deref(), &body))
            },
            Inline::Image { source, alt } => f.write_str(&Cf(source).render(alt)),
            Inline::SoftBreak => f.write_str("\n"),
            Inline::HardBreak => f.write_str("<br/>"),
            Inline::Macro(m) => write!(f, "{}", Cf(m)),
            Inline::RawConfluence(s) => f.write_str(s),
        }
    }
}

impl Cf<&LinkTarget> {
    fn render(&self, title: Option<&str>, body: &str) -> String {
        match self.0 {
            LinkTarget::External(url) => {
                let title_attr = title
                    .map(|t| format!(" title=\"{}\"", t.escape_attr()))
                    .unwrap_or_default();
                format!("<a href=\"{}\"{title_attr}>{body}</a>", url.escape_attr())
            },
            LinkTarget::Page {
                space,
                title: page,
                content_id,
            } => {
                let space_attr = space
                    .as_ref()
                    .map(|s| format!(" ri:space-key=\"{}\"", s.escape_attr()))
                    .unwrap_or_default();
                let id_attr = content_id
                    .as_ref()
                    .map(|id| format!(" ri:content-id=\"{}\"", id.escape_attr()))
                    .unwrap_or_default();
                format!(
                    "<ac:link><ri:page ri:content-title=\"{}\"{space_attr}{id_attr}/><ac:link-body>{body}</ac:link-body></ac:link>",
                    page.escape_attr()
                )
            },
            LinkTarget::Content(id) => format!(
                "<ac:link><ri:content-entity ri:content-id=\"{}\"/><ac:link-body>{body}</ac:link-body></ac:link>",
                id.escape_attr()
            ),
            LinkTarget::Attachment(file) => format!(
                "<ac:link><ri:attachment ri:filename=\"{}\"/><ac:link-body>{body}</ac:link-body></ac:link>",
                file.escape_attr()
            ),
            LinkTarget::Anchor(name) => format!(
                "<ac:link ac:anchor=\"{}\"><ac:link-body>{body}</ac:link-body></ac:link>",
                name.escape_attr()
            ),
        }
    }
}

impl Cf<&ImageSource> {
    fn render(&self, alt: &str) -> String {
        let alt = alt.escape_attr();
        match self.0 {
            ImageSource::External(url) => format!(
                "<ac:image ac:alt=\"{alt}\"><ri:url ri:value=\"{}\"/></ac:image>",
                url.escape_attr()
            ),
            ImageSource::Attachment(file) => format!(
                "<ac:image ac:alt=\"{alt}\"><ri:attachment ri:filename=\"{}\"/></ac:image>",
                file.escape_attr()
            ),
        }
    }
}

/// Confluence Storage Format (XHTML) escaping, as extension methods on `str`.
trait StrExt {
    /// Escapes XML text content (`&`, `<`, `>`).
    fn escape_text(&self) -> String;

    /// Escapes an XML attribute value: text plus `"`.
    fn escape_attr(&self) -> String;
}

impl StrExt for str {
    fn escape_text(&self) -> String {
        self.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn escape_attr(&self) -> String {
        self.escape_text().replace('"', "&quot;")
    }
}
