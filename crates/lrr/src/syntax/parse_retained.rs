//! Replay retained pinned syntax through the ordinary parser representation.
//! Source publication and expansion/seal authority are separate prerequisites.
use super::*;
use lrr_expand_adapter::{
    declaration_output::DeclarationGrammar, item_output::SyntaxEvent,
    syntax_attachment::SyntaxSnapshot,
};
use std::sync::Arc;

pub(super) struct RetainedSyntax(Arc<SyntaxSnapshot>);
impl std::fmt::Debug for RetainedSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedSyntax")
            .field("identity", &self.0.identity())
            .finish()
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-OUTPUT", message.into())
}

fn snapshot_bytes(snapshot: &SyntaxSnapshot) -> Result<usize, Diagnostic> {
    let syntax = snapshot.syntax();
    let add = |a: usize, b: usize| {
        a.checked_add(b)
            .filter(|n| *n <= 128 * 1024 * 1024)
            .ok_or_else(|| fault("retained syntax metadata budget"))
    };
    let array = |n: usize, size: usize| {
        n.checked_mul(size)
            .ok_or_else(|| fault("retained syntax metadata multiplication overflow"))
    };
    let mut bytes = add(
        std::mem::size_of::<SyntaxSnapshot>(),
        syntax.text.capacity(),
    )?;
    bytes = add(
        bytes,
        array(syntax.syntax.capacity(), std::mem::size_of::<SyntaxEvent>())?,
    )?;
    bytes = add(
        bytes,
        array(
            syntax.origins.capacity(),
            std::mem::size_of::<lrr_expand_adapter::item_output::Origin>(),
        )?,
    )?;
    bytes = add(
        bytes,
        array(
            syntax.definitions.capacity(),
            std::mem::size_of::<lrr_expand_adapter::declaration_output::DefinitionCapture>(),
        )?,
    )?;
    bytes = add(
        bytes,
        array(
            syntax.calls.capacity(),
            std::mem::size_of::<lrr_expand_adapter::declaration_output::CallCapture>(),
        )?,
    )?;
    let adapter = |e: lrr_expand_adapter::AdapterError| Diagnostic::new(e.code, e.message);
    bytes = add(
        bytes,
        snapshot
            .origin_records()
            .allocated_bytes()
            .map_err(adapter)?,
    )?;
    for event in &syntax.syntax {
        if let SyntaxEvent::Token { text, .. } = event {
            bytes = add(bytes, text.capacity())?;
        }
    }
    for definition in &syntax.definitions {
        bytes = add(bytes, definition.body.allocated_bytes().map_err(adapter)?)?;
    }
    for call in &syntax.calls {
        bytes = add(bytes, call.input.allocated_bytes().map_err(adapter)?)?;
        bytes = add(bytes, call.path.allocated_bytes().map_err(adapter)?)?;
    }
    Ok(bytes)
}

impl ParsedFile {
    /// Project diagnostic offsets through the exact retained token origins.
    /// Distinct locations are returned separately, with their complete context,
    /// owning edition and origin keys. This pure projection grants no source,
    /// name-resolution or execution authority. Original text uses SourceLease.
    pub fn retained_source_spans(
        &self,
        range: SourceRange,
    ) -> Result<Option<Vec<super::super::token_wire::SpanRecord>>, Diagnostic> {
        use super::super::token_wire::{self as owned, SpanRecord};
        let Some(retained) = &self.retained_syntax else {
            return Ok(None);
        };
        let snapshot = &retained.0;
        let syntax = snapshot.syntax();
        let spans = snapshot.origin_records().spans();
        let selected = super::super::spans::project_origin_spans(
            &syntax.text,
            &syntax.origins,
            range,
            |origin| (origin.start, origin.end),
            |origin| {
                spans
                    .binary_search_by_key(&origin.span, |span| span.key())
                    .map(|index| &spans[index])
                    .map_err(|_| {
                        Diagnostic::new(
                            "LRR-E-INTERNAL-SOURCE-MAP",
                            "retained diagnostic span absent".into(),
                        )
                    })
            },
        )?;
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "retained diagnostic span budget".into(),
            )
        };
        let mut output = Vec::new();
        output
            .try_reserve_exact(selected.len())
            .map_err(|_| limit())?;
        let mut bytes = output
            .capacity()
            .checked_mul(std::mem::size_of::<SpanRecord>())
            .ok_or_else(limit)?;
        for span in selected {
            bytes
                .checked_add(span.source.path.len())
                .filter(|n| *n <= 64 * 1024)
                .ok_or_else(limit)?;
            let mut path = String::new();
            path.try_reserve_exact(span.source.path.len())
                .map_err(|_| limit())?;
            path.push_str(&span.source.path);
            bytes = bytes
                .checked_add(path.capacity())
                .filter(|n| *n <= 64 * 1024)
                .ok_or_else(limit)?;
            let projected = SpanRecord {
                source: owned::SourceIdentity {
                    unit: owned::Key(span.source.unit.0),
                    content: owned::Key(span.source.content.0),
                    path,
                },
                start: span.start,
                end: span.end,
                context: owned::Key(span.context.0),
                edition: match span.edition {
                    lrr_expand_adapter::token_wire::Edition::E2015 => owned::Edition::E2015,
                    lrr_expand_adapter::token_wire::Edition::E2018 => owned::Edition::E2018,
                    lrr_expand_adapter::token_wire::Edition::E2021 => owned::Edition::E2021,
                    lrr_expand_adapter::token_wire::Edition::E2024 => owned::Edition::E2024,
                },
                origin: owned::Key(span.origin.0),
            };
            if projected.key().0 != span.key().0 {
                return Err(Diagnostic::new(
                    "LRR-E-INTERNAL-SOURCE-MAP",
                    "retained diagnostic span schema differs".into(),
                ));
            }
            output.push(projected);
        }
        Ok(Some(output))
    }

    pub(crate) fn snapshot_metadata_bytes(snapshot: &SyntaxSnapshot) -> Result<usize, Diagnostic> {
        snapshot_bytes(snapshot)
    }
    /// Pure representation conversion, never a source/namespace publication.
    /// The opaque snapshot supplies its exact structural identity and closed
    /// origin records. No caller-provided digest or fallback edition is used.
    pub fn from_syntax_snapshot(
        snapshot: Arc<SyntaxSnapshot>,
        profile: ProfileIdentity,
    ) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        if !matches!(snapshot.grammar(), DeclarationGrammar::Items) {
            return Err(fault("retained source unit requires item grammar"));
        }
        let syntax = snapshot.syntax();
        if syntax.syntax.is_empty()
            || syntax.syntax.len() > 6_000_000
            || syntax.text.len() > 32 * 1024 * 1024
        {
            return Err(fault("retained syntax event/text budget"));
        }
        let retained_bytes = snapshot_bytes(&snapshot)?;
        let mut builder = ra_ap_syntax::SyntaxTreeBuilder::default();
        let mut depth = 0usize;
        for (position, event) in syntax.syntax.iter().enumerate() {
            match event {
                SyntaxEvent::Start(raw) => {
                    if *raw >= ra_ap_syntax::SyntaxKind::__LAST as u16
                        || (depth == 0 && position != 0)
                    {
                        return Err(fault("retained syntax root/kind differs"));
                    }
                    let kind = ra_ap_syntax::SyntaxKind::from(*raw);
                    if position == 0 && kind != ra_ap_syntax::SyntaxKind::MACRO_ITEMS {
                        return Err(fault("retained syntax is not an item list"));
                    }
                    depth = depth
                        .checked_add(1)
                        .filter(|n| *n <= 4096)
                        .ok_or_else(|| fault("retained syntax depth budget"))?;
                    builder.start_node(if position == 0 {
                        ra_ap_syntax::SyntaxKind::SOURCE_FILE
                    } else {
                        kind
                    });
                }
                SyntaxEvent::Token { kind, text } => {
                    if depth == 0 || *kind >= ra_ap_syntax::SyntaxKind::__LAST as u16 {
                        return Err(fault("retained syntax token outside a valid node"));
                    }
                    builder.token(ra_ap_syntax::SyntaxKind::from(*kind), text);
                }
                SyntaxEvent::Finish => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| fault("retained syntax closing depth differs"))?;
                    builder.finish_node();
                }
            }
        }
        if depth != 0 {
            return Err(fault("retained syntax is incomplete"));
        }
        let parsed = builder
            .finish()
            .cast::<ast::SourceFile>()
            .ok_or_else(|| fault("retained source root differs"))?;
        if u32::from(parsed.syntax_node().text_range().len()) as usize != syntax.text.len() {
            return Err(fault("retained syntax text length differs"));
        }
        let estimated_bytes = syntax
            .syntax
            .len()
            .checked_mul(32)
            .and_then(|n| n.checked_add(syntax.text.len()))
            .and_then(|n| n.checked_add(retained_bytes))
            .filter(|n| *n <= 128 * 1024 * 1024)
            .ok_or_else(|| fault("retained source metadata budget"))?;
        let identity = Digest(snapshot.identity().0);
        let owned = Self {
            content: identity,
            edition: profile.edition,
            profile,
            errors: Vec::new(),
            parsed,
            generated_origin: Some(identity),
            declaration_grammar: Some(crate::expand::identity::GrammarSlot::Items),
            estimated_bytes,
            token_contexts: None,
            retained_syntax: None,
        }
        .with_generated_token_editions(snapshot.origin_records(), &syntax.origins)?;
        Ok(Self {
            retained_syntax: Some(RetainedSyntax(snapshot)),
            ..owned
        })
    }

    /// Exact retained representation bytes determine a body identity; display
    /// text alone cannot distinguish expansion grouping. Location/mark records
    /// remain dependencies of source publication, not absolute offsets in this
    /// body-local syntax identity. Ordinary UTF-8 body identities stay unchanged.
    pub(crate) fn body_content_identity(
        &self,
        root: &ra_ap_syntax::SyntaxNode,
    ) -> Result<Digest, Diagnostic> {
        if u32::from(root.text_range().len()) > 32 * 1024 * 1024 {
            return Err(fault("body representation text budget"));
        }
        if self.retained_syntax.is_none() {
            let text = root.text();
            let mut content = String::new();
            content
                .try_reserve_exact(usize::from(text.len()))
                .map_err(|_| fault("body content verification allocation"))?;
            text.for_each_chunk(|chunk| content.push_str(chunk));
            return Ok(Digest::of_bytes(content.as_bytes()));
        }
        let mut hash = StableEncoder::new("lrr/v2/retained-body-syntax");
        hash.string(lrr_expand_adapter::UPSTREAM_COMMIT);
        hash.record(&self.profile);
        for (count, event) in root.preorder_with_tokens().enumerate() {
            if count >= 6_000_000 {
                return Err(fault("body representation event budget"));
            }
            match event {
                ra_ap_syntax::WalkEvent::Enter(ra_ap_syntax::NodeOrToken::Node(node)) => {
                    hash.u8(0);
                    hash.u16(node.kind() as u16);
                }
                ra_ap_syntax::WalkEvent::Enter(ra_ap_syntax::NodeOrToken::Token(token)) => {
                    hash.u8(1);
                    hash.u16(token.kind() as u16);
                    hash.string(token.text());
                    if token.kind().is_trivia() {
                        hash.u8(0);
                    } else {
                        hash.u8(1);
                        self.encode_token_editions(range(token.text_range()), &mut hash)?;
                    }
                }
                ra_ap_syntax::WalkEvent::Leave(ra_ap_syntax::NodeOrToken::Node(_)) => hash.u8(2),
                ra_ap_syntax::WalkEvent::Leave(ra_ap_syntax::NodeOrToken::Token(_)) => {}
            }
        }
        Ok(hash.finish())
    }

    fn encode_token_editions(
        &self,
        range: SourceRange,
        encoder: &mut StableEncoder,
    ) -> Result<(), Diagnostic> {
        let editions = self
            .token_contexts
            .as_ref()
            .ok_or_else(|| fault("retained body edition map absent"))?;
        let mut index = editions.partition_point(|context| context.end <= range.start);
        let mut position = range.start;
        // A fused syntax token (e.g. ::) can cross origin intervals. Coalesce
        // equal owning editions, without interpreting punctuation or names.
        let mut run: Option<(u32, u16)> = None;
        while position < range.end {
            let context = editions
                .get(index)
                .ok_or_else(|| fault("retained body edition coverage incomplete"))?;
            let (end, edition) = (context.end, context.edition);
            let next = end.min(range.end);
            if next <= position {
                return Err(fault("retained body edition range differs"));
            }
            if let Some((run_end, previous)) = run
                && previous != edition
            {
                encoder.u32(run_end - range.start);
                encoder.u16(previous);
            }
            run = Some((next, edition));
            position = next;
            index += 1;
        }
        if let Some((end, edition)) = run {
            encoder.u32(end - range.start);
            encoder.u16(edition);
        }
        encoder.u32(u32::MAX); // Terminator cannot be a bounded body offset.
        Ok(())
    }
}

#[cfg(test)]
#[path = "parse_retained_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "retained_origin_tests.rs"]
mod origin_tests;
