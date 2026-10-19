//! Selected declaration headers from an actually admitted entered body.
//! This publishes source facts, never a name/scope seal or executable body.
use super::*;
use crate::{
    core::digest::StableEncode,
    frontend::source_body_context::PreparedBodyContext,
    syntax::{
        ast_adapter::tokens,
        entered_macros::DeclarationFacts,
        outline::{BodyHandle, DeclarationContext, ScopeKey},
        parse::ParsedFile,
    },
};
use ra_ap_syntax::{AstNode, TextRange, ast};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum LocalDeclarationPublication {
    Published(Digest),
    NeedsProducer(Digest),
}

#[derive(Clone, Copy)]
enum DeclarationAnnotationCompletion<'a> {
    Original(&'a crate::frontend::impl_index::CompletedOriginalAnnotation),
    Entered(&'a crate::frontend::source_body_context::PreparedDeclarationNameCompletions),
}
impl DeclarationAnnotationCompletion<'_> {
    fn require_local_declaration(self, parsed: &ParsedFile, body: &BodyHandle, scope: &ScopeKey,
        row: &crate::syntax::body_items::BodyItem) -> Result<(), Diagnostic> {
        match self {
            Self::Original(receipt) => receipt.require_local_declaration(parsed, body, scope, row),
            Self::Entered(receipt) => receipt.require_local_declaration(parsed, body, scope, row),
        }
    }
}

impl Database {
    /// Cold coordinator preparation. `definition` selects only among the real
    /// immutable entered facts; it supplies no declaration, name or schema.
    pub(crate) fn publish_local_declaration(
        &mut self,
        context: &PreparedBodyContext,
        definition: Digest,
    ) -> QueryPoll<LocalDeclarationPublication> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        if self.run_active {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "local declaration publication during run",
            )));
        }
        self.publish_local_declaration_checked(context, definition)
    }
    /// Shared exact-header publication after a cold or opaque reached-call
    /// coordinator boundary. It never prepares an executable declaration body.
    pub(super) fn publish_local_declaration_checked(
        &mut self,
        context: &PreparedBodyContext,
        definition: Digest,
    ) -> QueryPoll<LocalDeclarationPublication> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        match context.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let source = context.source();
        self.publish_declaration_metadata(
            source.parsed(),
            source.body(),
            source.scope(),
            context.facts().declaration_facts(),
            definition,
            None,
        )
    }
    /// Same metadata kernel after this admitted body's structural annotation join.
    pub(super) fn publish_local_declaration_with_names_checked(
        &mut self, context: &PreparedBodyContext, definition: Digest,
        names: &crate::frontend::source_body_context::PreparedDeclarationNameCompletions,
    ) -> QueryPoll<LocalDeclarationPublication> {
        if let Err(error) = self.require_coordinator().and_then(|()| names.require_context(context)) {
            return QueryPoll::Ready(Err(error));
        }
        match context.require_current(self) {
            QueryPoll::Ready(Ok(())) => {},
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let source = context.source();
        self.publish_declaration_metadata(source.parsed(), source.body(), source.scope(),
            context.facts().declaration_facts(), definition, Some(DeclarationAnnotationCompletion::Entered(names)))
    }
    /// The same exact-row publication kernel for a retained cold declaration
    /// scope. Its owner proof supplies no executable BodyIndex or bindings.
    pub(crate) fn publish_captured_declaration(
        &mut self,
        scope: &crate::frontend::associated_declarations::CapturedDeclarationScope<'_>,
        definition: Digest,
    ) -> QueryPoll<LocalDeclarationPublication> {
        self.publish_captured_declaration_using(scope, definition, None)
    }
    pub(crate) fn publish_captured_declaration_with_completion(
        &mut self,
        scope: &crate::frontend::associated_declarations::CapturedDeclarationScope<'_>,
        completion: &crate::frontend::impl_index::CompletedOriginalAnnotation,
    ) -> QueryPoll<LocalDeclarationPublication> {
        self.publish_captured_declaration_using(scope, completion.item(), Some(completion))
    }
    fn publish_captured_declaration_using(
        &mut self,
        scope: &crate::frontend::associated_declarations::CapturedDeclarationScope<'_>,
        definition: Digest,
        completion: Option<&crate::frontend::impl_index::CompletedOriginalAnnotation>,
    ) -> QueryPoll<LocalDeclarationPublication> {
        if self.run_active {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "cold declaration publication during run",
            )));
        }
        if let Err(error) = self
            .require_coordinator()
            .and_then(|()| scope.require_current(self))
        {
            return QueryPoll::Ready(Err(error));
        }
        if let Some(completion) = completion {
            if let Err(error) = completion.require_source(scope.source()) {
                return QueryPoll::Ready(Err(error));
            }
        }
        let parsed = match scope.parsed() {
            Ok(parsed) => parsed,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        self.publish_declaration_metadata(
            parsed,
            scope.body(),
            scope.scope(),
            scope.facts(),
            definition,
            completion.map(DeclarationAnnotationCompletion::Original),
        )
    }
    /// Bulk cold publication validates the owning scope once, then visits its
    /// actual canonical rows directly. It shares the exact row kernel with
    /// selected entered/cold publication rather than searching all rows per item.
    pub(crate) fn publish_captured_declarations(
        &mut self,
        scope: &crate::frontend::associated_declarations::CapturedDeclarationScope<'_>,
        completions: &BTreeMap<Digest, Arc<crate::frontend::impl_index::CompletedOriginalAnnotation>>,
    ) -> QueryPoll<Vec<LocalDeclarationPublication>> {
        QueryPoll::Ready((|| {
            self.require_coordinator()?;
            if self.run_active {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "cold declaration publication during run",
                ));
            }
            scope.require_current(self)?;
            let parsed = scope.parsed()?;
            let mut results = Vec::new();
            for (member, row) in scope
                .facts()
                .items
                .iter()
                .map(|row| (false, row))
                .chain(scope.facts().members.iter().map(|row| (true, row)))
            {
                let completion = completions.get(&row.item.def_key.fingerprint());
                if let Some(completion) = completion {
                    completion.require_source(scope.source())?;
                }
                results.push(self.publish_declaration_row_metadata(
                    parsed,
                    scope.body(),
                    scope.scope(),
                    row,
                    member,
                    completion.map(|receipt| DeclarationAnnotationCompletion::Original(&**receipt)),
                )?);
            }
            Ok(results)
        })())
    }
    fn publish_declaration_metadata(
        &mut self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        scope: &ScopeKey,
        facts: &DeclarationFacts,
        definition: Digest,
        completion: Option<DeclarationAnnotationCompletion<'_>>,
    ) -> QueryPoll<LocalDeclarationPublication> {
        QueryPoll::Ready((|| {
            let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
            let mut selected = facts
                .items
                .iter()
                .map(|row| (false, row))
                .chain(facts.members.iter().map(|row| (true, row)))
                .filter(|(_, row)| row.item.def_key.fingerprint() == definition);
            let (member, row) = selected
                .next()
                .ok_or_else(|| invalid("selected local declaration is absent from owning body"))?;
            if selected.next().is_some() {
                return Err(invalid("selected local declaration identity is ambiguous"));
            }
            self.publish_declaration_row_metadata(parsed, body, scope, row, member, completion)
        })())
    }
    fn publish_declaration_row_metadata(
        &mut self,
        parsed: &ParsedFile,
        body: &BodyHandle,
        scope: &ScopeKey,
        row: &crate::syntax::body_items::BodyItem,
        member: bool,
        completion: Option<DeclarationAnnotationCompletion<'_>>,
    ) -> Result<LocalDeclarationPublication, Diagnostic> {
        let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
        let definition = row.item.def_key.fingerprint();
        if let Some(producer) = row.attribute_input {
            match completion {
                Some(completion) => {
                    completion.require_local_declaration(parsed, body, scope, row)?
                }
                None => return Ok(LocalDeclarationPublication::NeedsProducer(producer)),
            }
        } else if completion.is_some() {
            return Err(invalid(
                "annotation completion supplied for an unannotated declaration",
            ));
        }
        let item = &row.item;
        if !body.range.contains(item.declaration_span)
            || item.header.profile != parsed.key().profile
            || item.scope.crate_key != scope.crate_key
            || item.scope.module_path != scope.module_path
            || item.def_key.crate_key != item.scope.crate_key
            || item.def_key.module_path != item.scope.module_path
        {
            return Err(invalid("selected local declaration owning source differs"));
        }
        // A block-local enum's discriminant constant is declared by its
        // variant (E-R5-013).
        let syntax = parsed
            .syntax_at(item.declaration_span, |node| {
                ast::Item::can_cast(node.kind()) || ast::Variant::can_cast(node.kind())
            })?
            .ok_or_else(|| invalid("selected local declaration exact syntax is absent"))?;
        let excluded = item
            .body
            .as_ref()
            .map(|body| body.block_range())
            .or(row.container_range)
            .map(|range| TextRange::new(range.start.into(), range.end.into()));
        if tokens(&syntax, excluded) != item.header.tokens {
            return Err(invalid(
                "selected local declaration header attachment differs",
            ));
        }
        let rows = [
            (
                InputKey::LocalDeclarationOwner(definition),
                InputValue::LocalDeclarationOwner(body.key()),
            ),
            (
                InputKey::DefinitionSource(definition),
                InputValue::DefinitionSource(parsed.key()),
            ),
            (
                InputKey::DefinitionHeader(definition),
                InputValue::DefinitionHeader(item.header.clone()),
            ),
            (
                InputKey::DeclarationContext(definition),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                }),
            ),
        ];
        let mut rows = rows.to_vec();
        // Member functions and const/static initializers are selected by
        // declaration identity. Retain their actual body locator without
        // preparing or executing the body; initialization is a reached VM
        // demand, just like a call to an impl member.
        if let Some(body) = item.body.as_ref()
            && (matches!(
                item.def_key.item_kind,
                crate::source::manifest::ItemKind::Constant
                    | crate::source::manifest::ItemKind::Static
            ) || member)
        {
            rows.push((
                InputKey::DefinitionBody(definition),
                InputValue::DefinitionBody(body.clone()),
            ));
        }
        let mut staged = BTreeMap::new();
        for (key, value) in rows {
            if let Some(current) = self.inputs.get(&key) {
                // Semantic header hashes deliberately omit attachments.
                // Compare the actual rows too; an equal hash cannot attach
                // this declaration to another source location or scope.
                let identical = match (current.value.as_ref(), &value) {
                    (
                        InputValue::LocalDeclarationOwner(a),
                        InputValue::LocalDeclarationOwner(b),
                    ) => a == b,
                    (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => a == b,
                    (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => a == b,
                    (InputValue::DeclarationContext(a), InputValue::DeclarationContext(b)) => {
                        a == b
                    }
                    (InputValue::DefinitionBody(a), InputValue::DefinitionBody(b)) => a == b,
                    _ => false,
                };
                if !identical || current.fingerprint != value.fingerprint() {
                    return Err(fault(
                        "LRR-E-EXPANSION-CONFLICT",
                        "selected local declaration publication conflicts with current rows",
                    ));
                }
                continue;
            }
            staged.insert(
                key,
                InputRecord {
                    fingerprint: value.fingerprint(),
                    value: Arc::new(value),
                },
            );
        }
        if staged.is_empty() {
            return Ok(LocalDeclarationPublication::Published(definition));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "local declaration revision exhausted"))?;
        self.inputs.extend(staged);
        self.revision = revision;
        Ok(LocalDeclarationPublication::Published(definition))
    }
}

#[cfg(test)]
#[path = "local_declaration_inputs_tests.rs"]
mod tests;
