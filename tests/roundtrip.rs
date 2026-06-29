//! Full round-trip matrix over every fixture: each pair must satisfy both
//! `md -> cf == xml` and `cf -> md == md`.

use confmark::Document;
use rstest::rstest;

fn fixture(name: &str, ext: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}.{ext}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {name}.{ext}: {e}"))
}

#[rstest]
#[case("headings")]
#[case("lists")]
#[case("code")]
#[case("mermaid")]
#[case("plantuml")]
#[case("links")]
#[case("link-page")]
#[case("link-page-id")]
#[case("link-attachment")]
#[case("link-anchor")]
#[case("link-content-id")]
#[case("image")]
#[case("image-attachment")]
#[case("blockquote")]
#[case("thematic-break")]
#[case("table")]
#[case("inline")]
#[case("autolink")]
#[case("tasklist")]
#[case("admonitions")]
#[case("status")]
#[case("toc")]
#[case("expand")]
#[case("panel")]
#[case("admonition-titled")]
#[case("unknown-macro")]
#[case("unknown-element")]
fn core_fixture_round_trips_both_directions(#[case] name: &str) {
    let md = fixture(name, "md");
    let md = md.trim_end_matches('\n');
    let xml = fixture(name, "xml");
    let xml = xml.trim_end_matches('\n');

    assert_eq!(Document::from_markdown(md).to_confluence(), xml, "md -> cf [{name}]");
    assert_eq!(Document::from_confluence(xml).to_markdown(), md, "cf -> md [{name}]");
}
