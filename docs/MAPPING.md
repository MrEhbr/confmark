# confmark conversion mapping

The contract between Markdown (CommonMark + GFM), the shared AST (`src/ast.rs`), and
Confluence Storage Format (XHTML). The parsers/renderers implement this document;
fixtures in `tests/fixtures/` are expected round-trip equivalents under it.

Namespaces: `ac:` = Confluence elements, `ri:` = resource identifiers.

## Block elements

| Markdown | AST `Block` | Confluence Storage |
|---|---|---|
| `# … ######` | `Heading { level, content }` | `<h1>…</h1>` … `<h6>…</h6>` |
| paragraph | `Paragraph(content)` | `<p>…</p>` |
| ` ```lang ` fenced | `CodeBlock { language, code }` | `code` macro (`ac:plain-text-body` CDATA, `language` param) |
| `-`/`1.` list | `List { ordered, items }` | `<ul>`/`<ol>` with `<li>` |
| `- [ ]`/`- [x]` | `TaskList(tasks)` | `<ac:task-list><ac:task>…` |
| `> …` | `BlockQuote(blocks)` | `<blockquote>…</blockquote>` |
| GFM table | `Table { align, head, rows }` | `<table><tbody><tr><th>/<td>…` |
| `---` | `ThematicBreak` | `<hr/>` |
| (see macros) | `Macro(m)` | `<ac:structured-macro>` |
| (unmappable) | `RawConfluence(s)` | preserved verbatim |

## Inline elements

| Markdown | AST `Inline` | Confluence Storage |
|---|---|---|
| text | `Text(s)` | text node |
| `**x**` | `Strong(content)` | `<strong>…</strong>` |
| `*x*` | `Emphasis(content)` | `<em>…</em>` |
| `~~x~~` | `Strikethrough(content)` | `<span style="text-decoration: line-through;">…</span>` |
| `` `x` `` | `Code(s)` | `<code>…</code>` |
| `[t](u "ti")` | `Link { target, title, content }` | external/resource link (see Links) |
| `![a](s)` | `Image { source, alt }` | `<ac:image …>` + `ri:url` or `ri:attachment` (see Links) |
| soft wrap | `SoftBreak` | newline / space |
| trailing `\` or two spaces | `HardBreak` | `<br/>` |
| (see macros) | `Macro(m)` | inline `<ac:structured-macro>` |
| (unmappable) | `RawConfluence(s)` | preserved verbatim |

## Links & images

A link's destination is a [`LinkTarget`]; an image's a [`ImageSource`]. External targets are
plain URLs; Confluence resource targets have no URL, so they round-trip through Markdown as a
reversible `confluence://` URI (query values are percent-encoded).

| AST `LinkTarget` | Markdown | Confluence Storage |
|---|---|---|
| `External(url)` | `[t](url)` | `<a href="url" [title]>t</a>` |
| `Page { space, title }` | `[t](confluence://page?space=SP&title=T)` | `<ac:link><ri:page ri:content-title="T" [ri:space-key="SP"]/><ac:link-body>t</ac:link-body></ac:link>` |
| `Content(id)` | `[t](confluence://content?id=N)` | `<ac:link><ri:content-entity ri:content-id="N"/><ac:link-body>t</ac:link-body></ac:link>` |
| `Attachment(file)` | `[t](confluence://attachment?file=F)` | `<ac:link><ri:attachment ri:filename="F"/><ac:link-body>t</ac:link-body></ac:link>` |
| `Anchor(name)` | `[t](confluence://anchor?name=N)` | `<ac:link ac:anchor="N"><ac:link-body>t</ac:link-body></ac:link>` |

Confluence Cloud also emits page-by-id links as `<ri:page ri:content-id="N"/>` (no
`ri:content-title`); these parse to `Content(id)` and normalize to `ri:content-entity` on render.

| AST `ImageSource` | Markdown | Confluence Storage |
|---|---|---|
| `External(url)` | `![a](url)` | `<ac:image ac:alt="a"><ri:url ri:value="url"/></ac:image>` |
| `Attachment(file)` | `![a](confluence://attachment?file=F)` | `<ac:image ac:alt="a"><ri:attachment ri:filename="F"/></ac:image>` |

Note: the Markdown link `title` is preserved only for external links (`<a title>`); Confluence
`ac:link` has no title slot.

## Macros

All Confluence macros are `<ac:structured-macro ac:name="NAME">` with zero or more
`<ac:parameter ac:name="K">V</ac:parameter>` and an optional body
(`<ac:rich-text-body>` nested storage, or `<ac:plain-text-body><![CDATA[…]]>`). They map
to `Macro { name, params, body }`. Markdown has no macro syntax, so each macro family has
a chosen Markdown representation below; unknown macros use the generic marker.

### Admonitions ↔ GFM alerts

`info`/`note`/`tip`/`warning` (body = `rich-text-body`) ↔ GFM alert blockquotes. The
alert token is chosen to round-trip bijectively and render as a styled callout on GitHub:

| Confluence `ac:name` | GFM alert |
|---|---|
| `note` | `> [!NOTE]` |
| `tip` | `> [!TIP]` |
| `warning` | `> [!WARNING]` |
| `info` | `> [!IMPORTANT]` |

Rationale: GitHub recognizes only NOTE/TIP/IMPORTANT/WARNING/CAUTION; `info` maps to
IMPORTANT (closest styled type) to keep the four-way mapping bijective. An admonition with a
`title` parameter has no alert representation (GFM alerts have no title slot), so it routes to
the paired generic marker instead (see below).

### expand ↔ `<details>`

`expand` (param `title`, body = `rich-text-body`) ↔ raw GFM-compatible HTML:

```html
<details><summary>TITLE</summary>

…body markdown…

</details>
```

### Generic marker for body-less and unknown macros

`status`, `toc`, `panel`, and any unknown macro use a reversible HTML comment carrying the
macro name and params (comrak surfaces these as HTML nodes, so no parser
extension is required). Grammar:

```
open        = "<!--cf:" name (" " param)* "-->"
close       = "<!--/cf:" name "-->"
param       = key "=" '"' value '"'
value-escape: '"' -> "&quot;", "-->" -> "--&gt;"
```

- **Inline, no body** (`status`): `<!--cf:status colour="Green" title="On track"-->`
- **Block, no body** (`toc`): `<!--cf:toc maxLevel="3"-->` on its own line.
- **Block, rich-text body** (`panel`, titled admonitions, unknown block macro):
  paired open/close markers wrapping the body:

  ```
  <!--cf:panel title="X"-->

  …body markdown…

  <!--/cf:panel-->
  ```

Titled admonitions (`note`/`tip`/`warning`/`info` with a `title` param) route to
the paired marker rather than a GFM alert (which has no title slot). The `code`
macro is the exception — it maps to a fenced code block, not a marker.

### RawConfluence (unrecognized markup)

Storage elements with no AST mapping (e.g. `<ac:layout>`, `<ac:emoticon>`) are
sliced from the source verbatim into `RawConfluence` and re-emitted unchanged on
`to_confluence`. In Markdown they round-trip as a `<!--cf-raw:…-->` comment
(`ac:`-namespaced tags are not valid CommonMark HTML, so the raw markup cannot be
passed through directly); `\n` and `-->` inside are escaped as `&#10;`/`--&gt;`.

## Known lossy points (v1)

- **Table alignment** (`Table.align`): storage format has no standard per-column alignment;
  alignment is dropped on md→cf and defaults to `None` on cf→md.
- **Macro styling params** (panel `bgColor`/`borderStyle`, status `subtle`, admonition
  `icon`): preserved as marker/param data where a marker is used; admonition styling beyond
  type is not expressible in GFM alerts.
- **Guarantee:** nothing is silently dropped — unmappable input becomes `RawConfluence` or a
  marker, never disappears.
