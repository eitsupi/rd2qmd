//! Document ordering and metadata extraction for the rd_ast migration.

use rd_ast::{RdArgument, RdDocument, RdNode};

use super::inline::{convert_inline_nodes, extract_plain_text};

/// The document-level information needed by later rendering steps.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DocumentStructure<'a> {
    pub(crate) title: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) sections: Vec<DocumentSection<'a>>,
}

/// A section in final output order, excluding the document title heading.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DocumentSection<'a> {
    Fixed(FixedSection<'a>),
    Custom { title: String, body: &'a [RdNode] },
}

/// One fixed-vocabulary Rd section and its unrendered body.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FixedSection<'a> {
    pub(crate) kind: FixedSectionKind,
    pub(crate) body: FixedSectionBody<'a>,
}

/// The body retained for a later section-specific conversion step.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FixedSectionBody<'a> {
    Nodes(&'a [RdNode]),
    Arguments(Vec<RdArgument<'a>>),
}

/// Fixed sections in pkgdown-compatible output order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixedSectionKind {
    Description,
    Usage,
    Arguments,
    Value,
    Details,
    Format,
    Source,
    Note,
    References,
    Author,
    SeeAlso,
    Examples,
}

impl FixedSectionKind {
    pub(crate) const fn heading(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Usage => "Usage",
            Self::Arguments => "Arguments",
            Self::Value => "Value",
            Self::Details => "Details",
            Self::Format => "Format",
            Self::Source => "Source",
            Self::Note => "Note",
            Self::References => "References",
            Self::Author => "Author",
            Self::SeeAlso => "See Also",
            Self::Examples => "Examples",
        }
    }
}

/// Metadata projected from a single Rd document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DocumentMetadata {
    pub(crate) aliases: Vec<String>,
    pub(crate) keywords: Vec<String>,
    pub(crate) concepts: Vec<String>,
    pub(crate) lifecycle: Option<String>,
    pub(crate) source_files: Vec<String>,
}

/// Build the title/name projection and section skeleton in final output order.
pub(crate) fn build_document_structure(document: &RdDocument) -> DocumentStructure<'_> {
    let mut sections = Vec::new();

    push_nodes(
        &mut sections,
        FixedSectionKind::Description,
        document.description(),
    );
    push_nodes(&mut sections, FixedSectionKind::Usage, document.usage());

    let arguments: Vec<_> = document.arguments().collect();
    if !arguments.is_empty() {
        sections.push(DocumentSection::Fixed(FixedSection {
            kind: FixedSectionKind::Arguments,
            body: FixedSectionBody::Arguments(arguments),
        }));
    }

    push_nodes(&mut sections, FixedSectionKind::Value, document.value());
    push_nodes(&mut sections, FixedSectionKind::Details, document.details());
    push_nodes(&mut sections, FixedSectionKind::Format, document.format());
    push_nodes(&mut sections, FixedSectionKind::Source, document.source());
    push_nodes(&mut sections, FixedSectionKind::Note, document.note());
    push_nodes(
        &mut sections,
        FixedSectionKind::References,
        document.references(),
    );
    push_nodes(&mut sections, FixedSectionKind::Author, document.author());
    push_nodes(
        &mut sections,
        FixedSectionKind::SeeAlso,
        document.see_also(),
    );

    sections.extend(document.sections().map(|section| DocumentSection::Custom {
        title: prose_text(section.title),
        body: section.body,
    }));

    push_nodes(
        &mut sections,
        FixedSectionKind::Examples,
        document.examples(),
    );

    DocumentStructure {
        title: document.title().map(prose_text),
        name: document.name().map(prose_text),
        sections,
    }
}

/// Extract sorted, deduplicated topic metadata and generation sources.
pub(crate) fn extract_document_metadata(document: &RdDocument) -> DocumentMetadata {
    let lifecycle_badges = document.lifecycle_badges();

    DocumentMetadata {
        aliases: sorted_unique(document.aliases()),
        keywords: sorted_unique(document.keywords()),
        concepts: sorted_unique(document.concepts()),
        lifecycle: lifecycle_badges
            .first()
            .map(|badge| badge.stage().as_str().to_owned()),
        source_files: crate::source_parse::extract_source_files(document),
    }
}

fn push_nodes<'a>(
    sections: &mut Vec<DocumentSection<'a>>,
    kind: FixedSectionKind,
    body: Option<&'a [RdNode]>,
) {
    if let Some(body) = body {
        sections.push(DocumentSection::Fixed(FixedSection {
            kind,
            body: FixedSectionBody::Nodes(body),
        }));
    }
}

fn prose_text(nodes: &[RdNode]) -> String {
    extract_plain_text(&convert_inline_nodes(nodes))
}

fn sorted_unique(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut values: Vec<_> = values
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use rd_ast::{RdDocument, RdNode, RdTag};

    use super::{
        DocumentSection, FixedSectionBody, FixedSectionKind, build_document_structure,
        extract_document_metadata,
    };

    fn tagged(tag: RdTag, text: &str) -> RdNode {
        RdNode::tagged(tag, None, vec![RdNode::Text(text.to_owned())])
    }

    fn argument(name: &str, description: &str) -> RdNode {
        RdNode::tagged(
            RdTag::Item,
            None,
            vec![
                RdNode::group(vec![RdNode::Text(name.to_owned())]),
                RdNode::group(vec![RdNode::Text(description.to_owned())]),
            ],
        )
    }

    fn custom_section(title: &str, body: &str) -> RdNode {
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![
                RdNode::group(vec![RdNode::Text(title.to_owned())]),
                RdNode::group(vec![RdNode::Text(body.to_owned())]),
            ],
        )
    }

    fn fixed_kinds(sections: &[DocumentSection<'_>]) -> Vec<FixedSectionKind> {
        sections
            .iter()
            .filter_map(|section| match section {
                DocumentSection::Fixed(section) => Some(section.kind),
                DocumentSection::Custom { .. } => None,
            })
            .collect()
    }

    #[test]
    fn fixed_sections_are_ordered_and_absent_sections_are_skipped() {
        let document = RdDocument::new(vec![
            tagged(RdTag::Note, "note"),
            tagged(RdTag::Value, "value"),
            RdNode::tagged(RdTag::Arguments, None, vec![argument("x", "the argument")]),
            tagged(RdTag::Source, "source"),
            tagged(RdTag::Description, "description"),
            tagged(RdTag::Examples, "example()"),
        ]);

        let structure = build_document_structure(&document);
        assert_eq!(
            fixed_kinds(&structure.sections),
            [
                FixedSectionKind::Description,
                FixedSectionKind::Arguments,
                FixedSectionKind::Value,
                FixedSectionKind::Source,
                FixedSectionKind::Note,
                FixedSectionKind::Examples,
            ]
        );
        assert!(!fixed_kinds(&structure.sections).contains(&FixedSectionKind::Details));

        let DocumentSection::Fixed(description) = &structure.sections[0] else {
            panic!("expected description section");
        };
        let FixedSectionBody::Nodes(body) = description.body else {
            panic!("expected node body");
        };
        assert!(std::ptr::eq(body, document.description().unwrap()));

        let DocumentSection::Fixed(arguments) = &structure.sections[1] else {
            panic!("expected arguments section");
        };
        let FixedSectionBody::Arguments(arguments) = &arguments.body else {
            panic!("expected argument views");
        };
        assert_eq!(arguments.len(), 1);
    }

    #[test]
    fn custom_sections_keep_source_order_before_examples() {
        let document = RdDocument::new(vec![
            tagged(RdTag::Examples, "example()"),
            custom_section("Second in output", "first custom body"),
            tagged(RdTag::Description, "description"),
            custom_section("Third in output", "second custom body"),
        ]);

        let structure = build_document_structure(&document);
        assert!(matches!(
            &structure.sections[0],
            DocumentSection::Fixed(section)
                if section.kind == FixedSectionKind::Description
        ));
        assert!(matches!(
            &structure.sections[1],
            DocumentSection::Custom { title, .. } if title == "Second in output"
        ));
        assert!(matches!(
            &structure.sections[2],
            DocumentSection::Custom { title, .. } if title == "Third in output"
        ));
        assert!(matches!(
            &structure.sections[3],
            DocumentSection::Fixed(section) if section.kind == FixedSectionKind::Examples
        ));

        let DocumentSection::Custom { body, .. } = &structure.sections[1] else {
            panic!("expected custom section");
        };
        assert!(std::ptr::eq(
            *body,
            document.sections().next().unwrap().body
        ));
    }

    #[test]
    fn extracts_trimmed_title_and_name_as_prose() {
        let document = RdDocument::new(vec![
            RdNode::tagged(
                RdTag::Title,
                None,
                vec![
                    RdNode::Text("  A ".to_owned()),
                    RdNode::group(vec![RdNode::RCode("mixed".to_owned())]),
                    RdNode::Verb(" title ".to_owned()),
                    RdNode::tagged(RdTag::R, None, vec![]),
                ],
            ),
            tagged(RdTag::Name, "  topic-name  "),
        ]);

        let structure = build_document_structure(&document);
        assert_eq!(structure.title.as_deref(), Some("A mixed title R"));
        assert_eq!(structure.name.as_deref(), Some("topic-name"));

        let special_character_document = RdDocument::new(vec![RdNode::tagged(
            RdTag::Title,
            None,
            vec![
                RdNode::Text("Using ".to_owned()),
                RdNode::tagged(RdTag::R, None, vec![]),
            ],
        )]);
        assert_eq!(
            build_document_structure(&special_character_document)
                .title
                .as_deref(),
            Some("Using R")
        );
    }

    #[test]
    fn exposes_legacy_heading_text() {
        let kinds = [
            FixedSectionKind::Description,
            FixedSectionKind::Usage,
            FixedSectionKind::Arguments,
            FixedSectionKind::Value,
            FixedSectionKind::Details,
            FixedSectionKind::Format,
            FixedSectionKind::Source,
            FixedSectionKind::Note,
            FixedSectionKind::References,
            FixedSectionKind::Author,
            FixedSectionKind::SeeAlso,
            FixedSectionKind::Examples,
        ];
        assert_eq!(
            kinds.map(FixedSectionKind::heading),
            [
                "Description",
                "Usage",
                "Arguments",
                "Value",
                "Details",
                "Format",
                "Source",
                "Note",
                "References",
                "Author",
                "See Also",
                "Examples",
            ]
        );
    }

    #[test]
    fn metadata_terms_are_trimmed_sorted_and_deduplicated() {
        let document = RdDocument::new(vec![
            tagged(RdTag::Alias, "zeta"),
            tagged(RdTag::Keyword, "models"),
            tagged(RdTag::Concept, "z concept"),
            tagged(RdTag::Alias, " alpha "),
            tagged(RdTag::Keyword, "data"),
            tagged(RdTag::Concept, "a concept"),
            tagged(RdTag::Alias, "alpha"),
            tagged(RdTag::Keyword, "models"),
            tagged(RdTag::Concept, "z concept"),
            tagged(RdTag::Alias, "  "),
        ]);

        let metadata = extract_document_metadata(&document);
        assert_eq!(metadata.aliases, ["alpha", "zeta"]);
        assert_eq!(metadata.keywords, ["data", "models"]);
        assert_eq!(metadata.concepts, ["a concept", "z concept"]);
    }

    #[test]
    fn extracts_first_lifecycle_badge_and_source_files() {
        let document = RdDocument::new(vec![
            RdNode::Comment("% Please edit documentation in R/topic.R".to_owned()),
            RdNode::tagged(
                RdTag::Description,
                None,
                vec![RdNode::tagged(
                    RdTag::Figure,
                    None,
                    vec![RdNode::group(vec![RdNode::Verb(
                        "lifecycle-stable.svg".to_owned(),
                    )])],
                )],
            ),
        ]);

        let metadata = extract_document_metadata(&document);
        assert_eq!(metadata.lifecycle.as_deref(), Some("stable"));
        assert_eq!(metadata.source_files, ["R/topic.R"]);
    }

    #[test]
    fn document_without_metadata_returns_empty_values() {
        let metadata = extract_document_metadata(&RdDocument::new(vec![]));

        assert!(metadata.aliases.is_empty());
        assert!(metadata.keywords.is_empty());
        assert!(metadata.concepts.is_empty());
        assert!(metadata.lifecycle.is_none());
        assert!(metadata.source_files.is_empty());
    }
}
