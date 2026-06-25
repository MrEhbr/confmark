use rstest::rstest;

use crate::ast::{Block, Document, ImageSource, Inline, LinkTarget};

fn doc(blocks: Vec<Block>) -> String {
    Document { blocks }.to_confluence()
}

fn text(s: &str) -> Inline {
    Inline::Text(s.to_string())
}

fn para_link(target: LinkTarget, label: &str) -> Vec<Block> {
    vec![Block::Paragraph(vec![Inline::Link {
        target,
        title: None,
        content: vec![text(label)],
    }])]
}

#[rstest]
#[case("headings.xml", vec![
        Block::Heading { level: 1, content: vec![text("Heading 1")] },
        Block::Heading { level: 2, content: vec![text("Heading 2")] },
        Block::Heading { level: 3, content: vec![text("Heading 3")] },
    ])]
#[case("code.xml", vec![
        Block::CodeBlock { language: Some("rust".into()), code: "fn main() {}".into() },
    ])]
#[case("links.xml", para_link(LinkTarget::External("https://www.atlassian.com".into()), "Atlassian"))]
#[case("link-page.xml", para_link(LinkTarget::Page { space: Some("SP".into()), title: "Home".into() }, "Home"))]
#[case("link-attachment.xml", para_link(LinkTarget::Attachment("a.pdf".into()), "doc"))]
#[case("link-anchor.xml", para_link(LinkTarget::Anchor("intro".into()), "intro"))]
#[case("image.xml", vec![
        Block::Paragraph(vec![Inline::Image { source: ImageSource::External("https://example.com/logo.png".into()), alt: "logo".into() }]),
    ])]
#[case("image-attachment.xml", vec![
        Block::Paragraph(vec![Inline::Image { source: ImageSource::Attachment("logo.png".into()), alt: "logo".into() }]),
    ])]
#[case("blockquote.xml", vec![
        Block::BlockQuote(vec![Block::Paragraph(vec![text("quoted text")])]),
    ])]
#[case("thematic-break.xml", vec![Block::ThematicBreak])]
fn renders_to_fixture(#[case] fixture: &str, #[case] blocks: Vec<Block>) {
    let expected = match fixture {
        "headings.xml" => include_str!("../../tests/fixtures/headings.xml"),
        "code.xml" => include_str!("../../tests/fixtures/code.xml"),
        "links.xml" => include_str!("../../tests/fixtures/links.xml"),
        "link-page.xml" => include_str!("../../tests/fixtures/link-page.xml"),
        "link-attachment.xml" => include_str!("../../tests/fixtures/link-attachment.xml"),
        "link-anchor.xml" => include_str!("../../tests/fixtures/link-anchor.xml"),
        "image.xml" => include_str!("../../tests/fixtures/image.xml"),
        "image-attachment.xml" => include_str!("../../tests/fixtures/image-attachment.xml"),
        "blockquote.xml" => include_str!("../../tests/fixtures/blockquote.xml"),
        "thematic-break.xml" => include_str!("../../tests/fixtures/thematic-break.xml"),
        _ => unreachable!(),
    };
    assert_eq!(doc(blocks), expected.trim_end_matches('\n'));
}

#[rstest]
fn renders_lists_fixture() {
    let nested = Block::List {
        ordered: false,
        items: vec![vec![Block::Paragraph(vec![text("nested")])]],
    };
    let ul = Block::List {
        ordered: false,
        items: vec![vec![Block::Paragraph(vec![text("one")])], vec![Block::Paragraph(vec![text("two")]), nested]],
    };
    let ol = Block::List {
        ordered: true,
        items: vec![vec![Block::Paragraph(vec![text("first")])], vec![Block::Paragraph(vec![text("second")])]],
    };
    let expected = include_str!("../../tests/fixtures/lists.xml").trim_end_matches('\n');
    assert_eq!(doc(vec![ul, ol]), expected);
}

#[rstest]
fn alignment_is_dropped_md_to_cf() {
    // Confluence storage has no per-column alignment, so md alignment is lost
    // on md->cf (and defaults to None on cf->md). One-way: aligned md -> the
    // same storage as the unaligned table fixture.
    let aligned = Document::from_markdown("| A | B |\n| :-- | --: |\n| 1 | 2 |");
    let expected = include_str!("../../tests/fixtures/table.xml").trim_end_matches('\n');
    assert_eq!(aligned.to_confluence(), expected);
}

#[rstest]
fn preserves_unknown_block_element_verbatim() {
    let xml = "<p>a</p>\n<ac:layout><ac:layout-section/></ac:layout>\n<p>b</p>";
    let parsed = Document::from_confluence(xml);
    assert_eq!(
        parsed.blocks,
        vec![
            Block::Paragraph(vec![text("a")]),
            Block::RawConfluence("<ac:layout><ac:layout-section/></ac:layout>".into()),
            Block::Paragraph(vec![text("b")]),
        ]
    );
    assert_eq!(parsed.to_confluence(), xml);
}

#[rstest]
fn preserves_unknown_inline_element_verbatim() {
    let xml = "<p>a <ac:emoticon ac:name=\"smile\"/> b</p>";
    let parsed = Document::from_confluence(xml);
    assert_eq!(
        parsed.blocks,
        vec![Block::Paragraph(vec![
            text("a "),
            Inline::RawConfluence("<ac:emoticon ac:name=\"smile\"/>".into()),
            text(" b"),
        ])]
    );
    assert_eq!(parsed.to_confluence(), xml);
}

#[rstest]
fn parses_diagram_macro_with_extra_attrs_and_empty_param() {
    // The macro as Confluence actually emits it: an `ac:macro-id`, an
    // `ac:schema-version`, and a stray empty `<ac:parameter ac:name=""/>` — all
    // of which must be tolerated and the CDATA body captured as a code block.
    let xml = concat!(
        "<ac:structured-macro ac:macro-id=\"abc-123\" ac:name=\"mermaiddiagram\" ac:schema-version=\"1\">\n",
        "  <ac:parameter ac:name=\"\"/>\n",
        "  <ac:plain-text-body><![CDATA[flowchart LR\n    A --> B]]></ac:plain-text-body>\n",
        "</ac:structured-macro>",
    );
    let parsed = Document::from_confluence(xml);
    assert_eq!(
        parsed.blocks,
        vec![Block::CodeBlock {
            language: Some("mermaid".into()),
            code: "flowchart LR\n    A --> B".into()
        }]
    );
}

#[rstest]
fn reassembles_split_cdata_diagram_body() {
    // Confluence splits the body around a literal `]]>` (here, an emoji) into
    // CDATA + text + CDATA. The three runs must concatenate back into one code
    // body and re-render as a single clean CDATA section.
    let xml = concat!(
        "<ac:structured-macro ac:name=\"mermaiddiagram\" ac:schema-version=\"1\">\n",
        "<ac:plain-text-body><![CDATA[A[\"]]>🔑<![CDATA[ B\"]]]></ac:plain-text-body>\n",
        "</ac:structured-macro>",
    );
    let parsed = Document::from_confluence(xml);
    assert_eq!(
        parsed.blocks,
        vec![Block::CodeBlock {
            language: Some("mermaid".into()),
            code: "A[\"🔑 B\"]".into()
        }]
    );
    assert_eq!(
        parsed.to_confluence(),
        concat!(
            "<ac:structured-macro ac:name=\"mermaiddiagram\" ac:schema-version=\"1\">\n",
            "<ac:plain-text-body><![CDATA[A[\"🔑 B\"]]]></ac:plain-text-body>\n",
            "</ac:structured-macro>",
        )
    );
}

#[rstest]
fn drops_unrepresentable_diagram_param_but_keeps_required_default() {
    // `theme` cannot ride through a Markdown fence; the registry's required
    // `atlassian-macro-output-type` always can.
    let xml = concat!(
        "<ac:structured-macro ac:name=\"plantuml\" ac:schema-version=\"1\">\n",
        "<ac:parameter ac:name=\"theme\">dark</ac:parameter>\n",
        "<ac:plain-text-body><![CDATA[A --> B]]></ac:plain-text-body>\n",
        "</ac:structured-macro>",
    );
    let parsed = Document::from_confluence(xml);
    assert_eq!(
        parsed.blocks,
        vec![Block::CodeBlock {
            language: Some("plantuml".into()),
            code: "A --> B".into()
        }]
    );
    assert_eq!(
        parsed.to_confluence(),
        concat!(
            "<ac:structured-macro ac:name=\"plantuml\" ac:schema-version=\"1\">\n",
            "<ac:parameter ac:name=\"atlassian-macro-output-type\">INLINE</ac:parameter>\n",
            "<ac:plain-text-body><![CDATA[A --> B]]></ac:plain-text-body>\n",
            "</ac:structured-macro>",
        )
    );
}

#[rstest]
fn escapes_text_and_attrs() {
    let p = Block::Paragraph(vec![
        text("a < b & c"),
        Inline::Link {
            target: LinkTarget::External("https://x.test?a=1&b=2".into()),
            title: Some("t\"q".into()),
            content: vec![text("link")],
        },
    ]);
    assert_eq!(
        doc(vec![p]),
        "<p>a &lt; b &amp; c<a href=\"https://x.test?a=1&amp;b=2\" title=\"t&quot;q\">link</a></p>"
    );
}
