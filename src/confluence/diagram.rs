//! Diagram macros: the bridge between a fenced code block and a named
//! Confluence structured-macro.
//!
//! A diagram (Mermaid, PlantUML, …) is modelled as an ordinary
//! [`Block::CodeBlock`](crate::ast::Block::CodeBlock) whose `language` names
//! the diagram type. On the Confluence side it renders as a dedicated
//! `<ac:structured-macro>` (e.g. `mermaiddiagram`) with the source in a CDATA
//! body, rather than as the generic `code` macro. The mapping is purely a
//! Confluence-side concern, so the Markdown parser/renderer and the shared AST
//! need no knowledge of it.
//!
//! To support a new diagram type, add a row to [`DiagramMacro::ALL`].

/// One entry in the diagram-macro registry: a Confluence structured-macro that
/// maps to a fenced code block of a given language.
pub(super) struct DiagramMacro {
    /// The Confluence `ac:name` (e.g. `mermaiddiagram`).
    pub name: &'static str,
    /// The Markdown fence language (e.g. `mermaid`).
    pub language: &'static str,
    /// The `ac:schema-version` to emit. A Markdown fence cannot carry it, so it
    /// is fixed per macro here and normalised on every md->cf render.
    pub schema_version: &'static str,
    /// `<ac:parameter>`s the macro needs to render correctly (e.g. PlantUML's
    /// `atlassian-macro-output-type=INLINE`). A Markdown fence cannot carry
    /// these, so they are fixed per macro here and emitted on every md->cf
    /// render; any other parameter on an incoming macro is dropped.
    pub params: &'static [(&'static str, &'static str)],
}

impl DiagramMacro {
    const ALL: &'static [DiagramMacro] = &[
        DiagramMacro {
            name: "mermaiddiagram",
            language: "mermaid",
            schema_version: "1",
            params: &[],
        },
        DiagramMacro {
            name: "plantuml",
            language: "plantuml",
            schema_version: "1",
            params: &[("atlassian-macro-output-type", "INLINE")],
        },
    ];

    /// The registered diagram macro for a Markdown fence language, if any.
    pub(super) fn for_language(language: &str) -> Option<&'static DiagramMacro> {
        Self::ALL.iter().find(|d| d.language == language)
    }

    /// The registered diagram macro for a Confluence `ac:name`, if any.
    pub(super) fn for_macro_name(name: &str) -> Option<&'static DiagramMacro> {
        Self::ALL.iter().find(|d| d.name == name)
    }
}
