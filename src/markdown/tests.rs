use rstest::rstest;

use crate::ast::{Alignment, Block, Document, ImageSource, Inline, LinkTarget, Macro, MacroBody, Table, Task};

fn doc(blocks: Vec<Block>) -> String {
    Document { blocks }.to_markdown()
}

fn text(s: &str) -> Inline {
    Inline::Text(s.to_string())
}

fn external(url: &str) -> LinkTarget {
    LinkTarget::External(url.to_string())
}

#[rstest]
#[case(Block::Heading { level: 1, content: vec![text("Heading 1")] }, "# Heading 1")]
#[case(Block::Heading { level: 3, content: vec![text("H3")] }, "### H3")]
#[case(Block::Paragraph(vec![text("plain")]), "plain")]
#[case(Block::ThematicBreak, "---")]
#[case(Block::CodeBlock { language: Some("rust".into()), code: "fn main() {}".into() }, "```rust\nfn main() {}\n```")]
fn renders_block(#[case] block: Block, #[case] expected: &str) {
    assert_eq!(doc(vec![block]), expected);
}

#[rstest]
#[case(Inline::Strong(vec![text("b")]), "**b**")]
#[case(Inline::Emphasis(vec![text("i")]), "*i*")]
#[case(Inline::Code("c".into()), "`c`")]
#[case(Inline::Link { target: external("https://x.test"), title: None, content: vec![text("L")] }, "[L](https://x.test)")]
#[case(Inline::Link { target: external("https://x.test"), title: Some("T".into()), content: vec![text("L")] }, "[L](https://x.test \"T\")")]
#[case(Inline::Image { source: ImageSource::External("i.png".into()), alt: "a".into() }, "![a](i.png)")]
fn renders_inline(#[case] inline: Inline, #[case] expected: &str) {
    assert_eq!(doc(vec![Block::Paragraph(vec![inline])]), expected);
}

#[rstest]
#[case(LinkTarget::Page { space: Some("SP".into()), title: "Home".into() }, "confluence://page?space=SP&title=Home")]
#[case(LinkTarget::Page { space: None, title: "On Track".into() }, "confluence://page?title=On%20Track")]
#[case(LinkTarget::Content("12345".into()), "confluence://content?id=12345")]
#[case(LinkTarget::Attachment("a.pdf".into()), "confluence://attachment?file=a.pdf")]
#[case(LinkTarget::Anchor("intro".into()), "confluence://anchor?name=intro")]
fn link_target_uri_roundtrips(#[case] target: LinkTarget, #[case] url: &str) {
    let link = Inline::Link {
        target: target.clone(),
        title: None,
        content: vec![text("x")],
    };
    assert_eq!(doc(vec![Block::Paragraph(vec![link])]), format!("[x]({url})"));
    assert_eq!(LinkTarget::from_url(url), target);
}

#[rstest]
fn parses_heading_and_paragraph() {
    let parsed = Document::from_markdown("# Title\n\nA paragraph.");
    assert_eq!(
        parsed.blocks,
        vec![
            Block::Heading {
                level: 1,
                content: vec![text("Title")]
            },
            Block::Paragraph(vec![text("A paragraph.")]),
        ]
    );
}

#[rstest]
fn parses_fenced_code_with_language() {
    let parsed = Document::from_markdown("```rust\nfn main() {}\n```");
    assert_eq!(
        parsed.blocks,
        vec![Block::CodeBlock {
            language: Some("rust".into()),
            code: "fn main() {}".into()
        }]
    );
}

#[rstest]
fn parses_internal_page_link() {
    let parsed = Document::from_markdown("[Home](confluence://page?space=SP&title=Home)");
    assert_eq!(
        parsed.blocks,
        vec![Block::Paragraph(vec![Inline::Link {
            target: LinkTarget::Page {
                space: Some("SP".into()),
                title: "Home".into()
            },
            title: None,
            content: vec![text("Home")],
        }])]
    );
}

#[rstest]
fn parses_nested_tight_list() {
    let parsed = Document::from_markdown("- one\n- two\n  - nested");
    let nested = Block::List {
        ordered: false,
        items: vec![vec![Block::Paragraph(vec![text("nested")])]],
    };
    assert_eq!(
        parsed.blocks,
        vec![Block::List {
            ordered: false,
            items: vec![vec![Block::Paragraph(vec![text("one")])], vec![Block::Paragraph(vec![text("two")]), nested]],
        }]
    );
}

#[rstest]
fn renders_inline_status_marker() {
    let m = Macro {
        name: "status".into(),
        params: vec![("colour".into(), "Green".into()), ("title".into(), "On track".into())],
        body: MacroBody::Empty,
    };
    let p = Block::Paragraph(vec![text("Status: "), Inline::Macro(m)]);
    assert_eq!(doc(vec![p]), "Status: <!--cf:status colour=\"Green\" title=\"On track\"-->");
}

#[rstest]
fn renders_block_toc_marker() {
    let m = Macro {
        name: "toc".into(),
        params: vec![("maxLevel".into(), "3".into())],
        body: MacroBody::Empty,
    };
    assert_eq!(doc(vec![Block::Macro(m)]), "<!--cf:toc maxLevel=\"3\"-->");
}

#[rstest]
fn renders_expand_details() {
    let m = Macro {
        name: "expand".into(),
        params: vec![("title".into(), "More".into())],
        body: MacroBody::RichText(vec![Block::Paragraph(vec![text("hidden content")])]),
    };
    assert_eq!(
        doc(vec![Block::Macro(m)]),
        "<details><summary>More</summary>\n\nhidden content\n\n</details>"
    );
}

#[rstest]
fn parses_inline_status_marker() {
    let parsed = Document::from_markdown("Status: <!--cf:status colour=\"Green\"-->");
    assert_eq!(
        parsed.blocks,
        vec![Block::Paragraph(vec![
            text("Status: "),
            Inline::Macro(Macro {
                name: "status".into(),
                params: vec![("colour".into(), "Green".into())],
                body: MacroBody::Empty,
            }),
        ])]
    );
}

#[rstest]
fn parses_block_toc_marker() {
    let parsed = Document::from_markdown("<!--cf:toc maxLevel=\"3\"-->");
    assert_eq!(
        parsed.blocks,
        vec![Block::Macro(Macro {
            name: "toc".into(),
            params: vec![("maxLevel".into(), "3".into())],
            body: MacroBody::Empty,
        })]
    );
}

#[rstest]
fn parses_expand_details() {
    let parsed = Document::from_markdown("<details><summary>More</summary>\n\nhidden content\n\n</details>");
    assert_eq!(
        parsed.blocks,
        vec![Block::Macro(Macro {
            name: "expand".into(),
            params: vec![("title".into(), "More".into())],
            body: MacroBody::RichText(vec![Block::Paragraph(vec![text("hidden content")])]),
        })]
    );
}

#[rstest]
fn marker_value_escaping_roundtrips() {
    // `"` and `-->` in a param value must survive the marker round-trip.
    let m = Macro {
        name: "status".into(),
        params: vec![("title".into(), "a \"b\" --> c".into())],
        body: MacroBody::Empty,
    };
    let parsed = Document::from_markdown(&doc(vec![Block::Macro(m.clone())]));
    assert_eq!(parsed.blocks, vec![Block::Macro(m)]);
}

#[rstest]
fn renders_paired_marker() {
    let m = Macro {
        name: "panel".into(),
        params: vec![("title".into(), "Notes".into())],
        body: MacroBody::RichText(vec![Block::Paragraph(vec![text("body")])]),
    };
    assert_eq!(
        doc(vec![Block::Macro(m)]),
        "<!--cf:panel title=\"Notes\"-->\n\nbody\n\n<!--/cf:panel-->"
    );
}

#[rstest]
fn titled_admonition_uses_paired_marker() {
    let m = Macro {
        name: "note".into(),
        params: vec![("title".into(), "Heads up".into())],
        body: MacroBody::RichText(vec![Block::Paragraph(vec![text("read this")])]),
    };
    assert_eq!(
        doc(vec![Block::Macro(m)]),
        "<!--cf:note title=\"Heads up\"-->\n\nread this\n\n<!--/cf:note-->"
    );
}

#[rstest]
fn parses_paired_marker_body() {
    let parsed = Document::from_markdown("<!--cf:panel title=\"Notes\"-->\n\nbody\n\n<!--/cf:panel-->");
    assert_eq!(
        parsed.blocks,
        vec![Block::Macro(Macro {
            name: "panel".into(),
            params: vec![("title".into(), "Notes".into())],
            body: MacroBody::RichText(vec![Block::Paragraph(vec![text("body")])]),
        })]
    );
}

#[rstest]
fn preserves_raw_confluence_block() {
    let raw = Block::RawConfluence("<ac:placeholder>x</ac:placeholder>".into());
    let md = doc(vec![raw.clone()]);
    assert_eq!(md, "<!--cf-raw:<ac:placeholder>x</ac:placeholder>-->");
    assert_eq!(Document::from_markdown(&md).blocks, vec![raw]);
}

#[rstest]
fn parses_admonition_alert() {
    let parsed = Document::from_markdown("> [!WARNING]\n> careful");
    assert_eq!(
        parsed.blocks,
        vec![Block::Macro(crate::ast::Macro {
            name: "warning".into(),
            params: vec![],
            body: crate::ast::MacroBody::RichText(vec![Block::Paragraph(vec![text("careful")])]),
        })]
    );
}

#[rstest]
fn plain_blockquote_is_not_an_alert() {
    let parsed = Document::from_markdown("> just a quote");
    assert_eq!(
        parsed.blocks,
        vec![Block::BlockQuote(vec![Block::Paragraph(vec![text("just a quote")])])]
    );
}

#[rstest]
fn parses_task_list() {
    let parsed = Document::from_markdown("- [ ] todo\n- [x] done");
    assert_eq!(
        parsed.blocks,
        vec![Block::TaskList(vec![
            Task {
                checked: false,
                content: vec![text("todo")]
            },
            Task {
                checked: true,
                content: vec![text("done")]
            },
        ])]
    );
}

#[rstest]
fn renders_strikethrough() {
    let p = Block::Paragraph(vec![Inline::Strikethrough(vec![text("gone")])]);
    assert_eq!(doc(vec![p]), "~~gone~~");
}

#[rstest]
fn parses_strikethrough() {
    let parsed = Document::from_markdown("~~gone~~");
    assert_eq!(
        parsed.blocks,
        vec![Block::Paragraph(vec![Inline::Strikethrough(vec![text("gone")])])]
    );
}

#[rstest]
fn autolink_normalizes_to_inline_link() {
    let parsed = Document::from_markdown("<https://example.com>");
    assert_eq!(parsed.to_markdown(), "[https://example.com](https://example.com)");
}

#[rstest]
fn parses_table_with_alignment() {
    let parsed = Document::from_markdown("| A | B |\n| :-- | --: |\n| 1 | 2 |");
    assert_eq!(
        parsed.blocks,
        vec![Block::Table(Table {
            align: vec![Alignment::Left, Alignment::Right],
            head: vec![vec![text("A")], vec![text("B")]],
            rows: vec![vec![vec![text("1")], vec![text("2")]]],
        })]
    );
}

#[rstest]
fn renders_table_alignment_delimiters() {
    let table = Block::Table(Table {
        align: vec![Alignment::Left, Alignment::Center, Alignment::Right, Alignment::None],
        head: vec![vec![text("a")], vec![text("b")], vec![text("c")], vec![text("d")]],
        rows: vec![],
    });
    assert_eq!(doc(vec![table]), "| a | b | c | d |\n| :-- | :-: | --: | --- |");
}

#[rstest]
#[case("# Heading 1\n\n## Heading 2")]
#[case("- one\n- two\n  - nested")]
#[case("> quoted text")]
#[case("```rust\nfn main() {}\n```")]
#[case("[Atlassian](https://www.atlassian.com)")]
#[case("[Home](confluence://page?space=SP&title=Home)")]
#[case("[doc](confluence://attachment?file=a.pdf)")]
fn markdown_roundtrips(#[case] md: &str) {
    assert_eq!(Document::from_markdown(md).to_markdown(), md);
}

#[rstest]
fn escapes_inline_markdown_chars() {
    let md = doc(vec![Block::Paragraph(vec![text("a*b_c~d`e[f]g<h")])]);
    assert_eq!(md, r"a\*b\_c\~d\`e\[f\]g\<h");
    // The escaped text must parse back to the same literal and re-render
    // identically.
    assert_eq!(Document::from_markdown(&md).to_markdown(), md);
}
