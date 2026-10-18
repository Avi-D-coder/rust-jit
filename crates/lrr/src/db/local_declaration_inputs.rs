//! Selected declaration headers from an actually admitted entered body.
//! This publishes source facts, never a name/scope seal or executable body.
use super::*;
use crate::{
    core::digest::StableEncode,
    frontend::source_body_context::PreparedBodyContext,
    syntax::{ast_adapter::tokens, outline::DeclarationContext},
};
use ra_ap_syntax::{AstNode, TextRange, ast};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum LocalDeclarationPublication {
    Published(Digest),
    NeedsProducer(Digest),
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
        QueryPoll::Ready((|| {
            let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
            let mut selected = context
                .facts()
                .declarations()
                .filter(|row| row.item.def_key.fingerprint() == definition);
            let row = selected
                .next()
                .ok_or_else(|| invalid("selected local declaration is absent from owning body"))?;
            if selected.next().is_some() {
                return Err(invalid("selected local declaration identity is ambiguous"));
            }
            if let Some(producer) = row.attribute_input {
                return Ok(LocalDeclarationPublication::NeedsProducer(producer));
            }
            let source = context.source();
            let item = &row.item;
            if !source.body().range.contains(item.declaration_span)
                || item.header.profile != source.parsed().key().profile
                || item.scope.crate_key != source.scope().crate_key
                || item.scope.module_path != source.scope().module_path
                || item.def_key.crate_key != item.scope.crate_key
                || item.def_key.module_path != item.scope.module_path
            {
                return Err(invalid("selected local declaration owning source differs"));
            }
            // A block-local enum's discriminant constant is declared by its
            // variant (E-R5-013).
            let syntax = source
                .parsed()
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
                    InputValue::LocalDeclarationOwner(source.index().body().clone()),
                ),
                (
                    InputKey::DefinitionSource(definition),
                    InputValue::DefinitionSource(source.parsed().key()),
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
                ) || context
                    .facts()
                    .members
                    .iter()
                    .any(|member| std::ptr::eq(member, row)))
            {
                rows.push((
                    InputKey::DefinitionBody(definition),
                    InputValue::DefinitionBody(body.clone()),
                ));
            }
            let mut staged = BTreeMap::new();
            let mut bytes = 0;
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
                        (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => {
                            a == b
                        }
                        (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => {
                            a == b
                        }
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
                bytes = account_add(bytes, input_metadata_bytes(&key, &value)?)?;
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
            let _ = bytes; // E-R5-006: no byte accounting.
            let revision = self.revision.checked_add(1).ok_or_else(|| {
                fault("LRR-E-ID-EXHAUSTED", "local declaration revision exhausted")
            })?;
            self.inputs.extend(staged);
            self.revision = revision;
            Ok(LocalDeclarationPublication::Published(definition))
        })())
    }
}

#[cfg(test)]
#[path = "local_declaration_inputs_tests.rs"]
mod tests;
