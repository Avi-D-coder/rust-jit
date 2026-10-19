//! Physical source locations for the pinned builtin adapter. Raw token context
//! ancestry selects each expansion's actual call site; no runtime Rust executes.
use super::{
    IncludedSourceViews, SourceDeclarative, builtin_call_site, builtin_call_site_in_source, fault,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    frontend::impl_index::{DeclarationCallSite, GeneratedDeclarationMacroIndex},
};
use lrr_expand_adapter::token_wire::{ContextRecord, SpanRecord, TokenForest, TokenTree};

fn path_site(path: &TokenForest) -> Result<(&SpanRecord, &ContextRecord), Diagnostic> {
    let token = path
        .roots()
        .first()
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "builtin invocation path absent"))?;
    let key = match token {
        TokenTree::Group { open, .. } => open,
        TokenTree::Ident { span, .. }
        | TokenTree::Punct { span, .. }
        | TokenTree::Literal { span, .. } => span,
    };
    let span = path
        .span_by_key(key)
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "builtin invocation span absent"))?;
    let context = path
        .contexts()
        .iter()
        .find(|context| context.key() == span.context)
        .ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "builtin invocation raw context absent",
            )
        })?;
    Ok((span, context))
}
/// Location metadata only. Names/editions still require their ordinary hygiene
/// joins. These views come solely from successful coordinator include admission.
fn included_source<'a>(
    included: Option<&'a IncludedSourceViews>,
    span: &SpanRecord,
) -> Result<Option<&'a crate::source::include_view::IncludedSourceView>, Diagnostic> {
    let Some(included) = included else {
        return Ok(None);
    };
    let mut matching = included.values().filter(|view| {
        view.location().unit.0 == span.source.unit.0
            && view.blob().content().0 == span.source.content.0
            && view.location().path.as_str() == span.source.path
            && view.origin().0 == span.origin.0
    });
    let Some(view) = matching.next() else {
        return Ok(None);
    };
    if matching.any(|other| other.identity() != view.identity()) {
        return Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "builtin included physical source is ambiguous",
        ));
    }
    if !view.tokens().spans().iter().any(|token| {
        token.source.unit.0 == span.source.unit.0
            && token.source.content.0 == span.source.content.0
            && token.source.path == span.source.path
            && token.origin.0 == span.origin.0
            && token.start == span.start
            && token.end == span.end
    }) {
        return Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "builtin included location lacks its actual captured token",
        ));
    }
    Ok(Some(view.as_ref()))
}

impl SourceDeclarative {
    pub(super) fn generated_builtin_call_site(
        &self,
        index: &GeneratedDeclarationMacroIndex,
        path: &TokenForest,
        call_edition: u16,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<lrr_expand_adapter::builtin_macros::BuiltinCallSite, Diagnostic> {
        self.generated_builtin_call_site_with_sources(index, path, call_edition, scope, None)
    }
    pub(super) fn generated_builtin_call_site_with_sources(
        &self,
        index: &GeneratedDeclarationMacroIndex,
        path: &TokenForest,
        call_edition: u16,
        scope: &crate::syntax::outline::ScopeKey,
        included: Option<&IncludedSourceViews>,
    ) -> Result<lrr_expand_adapter::builtin_macros::BuiltinCallSite, Diagnostic> {
        let (mut span, mut context) = path_site(path)?;
        // Each selected call context comes from an older admitted expansion.
        // Validated token contexts/parent graphs are acyclic; no new depth cap.
        let original;
        let mut visited = std::collections::BTreeSet::new();
        loop {
            // rustc's cause walk stops at include. The admitted view retains
            // that boundary even when the included text inherited a marked
            // caller context; a later macro mark must still be followed first.
            if let Some(view) = included_source(included, span)?
                && view.caller_context().0 == span.context.0
            {
                return builtin_call_site_in_source(
                    &self.source,
                    view.blob().text()?,
                    view.location().path.as_str(),
                    span.start,
                    call_edition,
                    scope,
                );
            }
            let Some(expansion) = context.expansion else {
                break;
            };
            if !visited.insert(expansion) {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "builtin expansion cause ancestry is cyclic",
                ));
            }
            match index.expansion_call_site(Digest(expansion.0))? {
                DeclarationCallSite::Captured(path) => {
                    (span, context) = path_site(path)?;
                }
                DeclarationCallSite::Source { scan, range } => {
                    let source = self.source.selection();
                    if scan.parse() != &source.outline.parse
                        || scan.path() != &source.path
                        || scan.scope() != &self.source.outline().root_scope
                        || scan.cfg() != source.outline.cfg
                    {
                        return Err(fault(
                            "LRR-E-EXPANSION-INPUT",
                            "builtin expansion cause source owner differs",
                        ));
                    }
                    // Use the captured lexical token, including its raw
                    // context. An argument delimiter is not a macro's start.
                    original = self.owned_forest()?;
                    let found = original
                        .spans()
                        .iter()
                        .find(|token| {
                            token.start == range.start
                                && token.source.unit.0 == source.outline.crate_key.0
                                && token.source.content.0 == source.outline.parse.content.0
                                && token.source.path == source.path.as_str()
                        })
                        .ok_or_else(|| {
                            fault(
                                "LRR-E-EXPANSION-INPUT",
                                "builtin original call token absent",
                            )
                        })?;
                    let raw = original
                        .contexts()
                        .iter()
                        .find(|row| row.key() == found.context)
                        .ok_or_else(|| {
                            fault(
                                "LRR-E-EXPANSION-INPUT",
                                "builtin original call context absent",
                            )
                        })?;
                    if raw.expansion.is_some() {
                        return Err(fault(
                            "LRR-E-EXPANSION-INPUT",
                            "builtin original call context requires an uncaptured expansion",
                        ));
                    }
                    span = found;
                    break;
                }
            }
        }
        let source = self.source.selection();
        if span.source.unit.0 != source.outline.crate_key.0
            || span.source.content.0 != source.outline.parse.content.0
            || span.source.path != source.path.as_str()
        {
            if let Some(view) = included_source(included, span)? {
                return builtin_call_site_in_source(
                    &self.source,
                    view.blob().text()?,
                    view.location().path.as_str(),
                    span.start,
                    call_edition,
                    scope,
                );
            }
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "builtin expansion cause physical source is not captured by its owner",
            ));
        }
        builtin_call_site(&self.source, span.start, call_edition, scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::source_declarative::assembly::tests::{output, request, world};
    use crate::frontend::impl_index::DeclarationMacroParent;

    const ORIGINAL: &str = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/issue-26322.rs"
    );

    #[test]
    fn builtin_cause_uses_actual_captured_index_and_rejects_foreign_source_or_expansion() {
        // Source/authority controls over the unchanged upstream producer. No
        // guest pass is inferred from preparing its captured expansion here.
        for edition in [2015, 2018, 2021, 2024] {
            let (provider, source, _) = world(ORIGINAL, edition);
            let issued = request(&provider, &source);
            let generated = output(&provider, source.scan(), &issued, 1);
            let index = GeneratedDeclarationMacroIndex::new(
                generated.clone(),
                DeclarationMacroParent::Source(source),
            )
            .unwrap();
            let invocation = generated
                .scan()
                .producers()
                .iter()
                .find(|producer| {
                    producer.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                })
                .unwrap()
                .identity;
            let path = &generated.macro_call(invocation).unwrap().path;
            let facts = provider
                .generated_builtin_call_site(&index, path, edition, generated.scan().scope())
                .unwrap();
            assert_eq!((facts.column, facts.line), (9, 19));
            let changed = format!("\n{ORIGINAL}");
            let (foreign, source, _) = world(&changed, edition);
            let issued = request(&foreign, &source);
            let generated = output(&foreign, source.scan(), &issued, 1);
            let foreign_index = GeneratedDeclarationMacroIndex::new(
                generated,
                DeclarationMacroParent::Source(source),
            )
            .unwrap();
            assert!(
                foreign
                    .generated_builtin_call_site(
                        &index,
                        path,
                        edition,
                        index.output().scan().scope()
                    )
                    .is_err(),
                "another physical source supplied the builtin coordinates"
            );
            assert!(
                provider
                    .generated_builtin_call_site(
                        &foreign_index,
                        path,
                        edition,
                        index.output().scan().scope()
                    )
                    .is_err(),
                "another actual expansion supplied the context-to-invocation join"
            );
        }
    }
}
