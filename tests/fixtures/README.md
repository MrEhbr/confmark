# Conversion fixtures

Each construct group is a pair: `<group>.md` (Markdown, CommonMark + GFM) and
`<group>.xml` (Confluence Storage Format). The two are **expected round-trip
equivalents** under the contract in [`docs/MAPPING.md`](../../docs/MAPPING.md):
`md → cf` should produce the `.xml`, and `cf → md` should produce the `.md`.

The `.xml` files are storage-format **fragments** — the body content as it appears
inside a Confluence page, using the `ac:`/`ri:` prefixes (whose namespaces Confluence
declares on the page wrapper). They are intentionally not standalone XML documents
(a fragment may have several top-level elements), so don't validate them as whole
documents — parse them as storage-format body fragments.

## Groups

`headings`, `inline`, `lists`, `tasklist`, `table`, `code`, `links`, `image`,
`blockquote`, `thematic-break`, `admonitions`, `expand`, `status`, `toc`, `panel`,
`unknown-macro`.

## Adding real exported pages

1. In Confluence, view/export the page storage format and copy the body fragment.
2. Save it as `tests/fixtures/<name>.xml`.
3. Author the expected Markdown as `tests/fixtures/<name>.md` per `docs/MAPPING.md`.

Phase 3 adds the round-trip test harness that loads these pairs; this directory is
data only.
