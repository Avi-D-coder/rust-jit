//! Structural function ownership for pre-entry assembly demand. No name/type
//! inference, executable shape checking, expansion or guest mutation occurs here.
use super::*;
use crate::{source::manifest::ItemKind, syntax::parse::ParsedFile};
use lrr_expand_adapter::syntax_attachment::BodyOwners;

fn item_owns_call(
    parsed: &ParsedFile,
    body: SourceRange,
    call: &CallCapture,
) -> Result<bool, Diagnostic> {
    let call_range = SourceRange::checked(call.start, call.end)?;
    Ok(body.contains(call_range) && parsed.macro_call_item_body(call_range)? == body)
}

impl PreparedSourceSyntax {
    /// Cold structural closure proof for an actually selected local declaration body.
    /// The caller must separately prove the declaration, source and parent
    /// authority. This does not broaden the assembly's scoped body permit.
    pub(crate) fn require_closed_item_body(
        &self,
        parsed: &ParsedFile,
        body: SourceRange,
    ) -> Result<Digest, Diagnostic> {
        if parsed.retained_syntax_identity() != Some(Digest(self.syntax.identity().0))
            || body.is_empty()
            || body.end as usize > self.syntax.syntax().text.len()
        {
            return Err(fault("local readiness retained tree/body differs"));
        }
        for call in &self.syntax.syntax().calls {
            if item_owns_call(parsed, body, call)?
                && self.cfg_disabled.binary_search(&anchor(call)?).is_err()
            {
                return Err(fault("local body still has pending expansion calls"));
            }
        }
        let mut identity = StableEncoder::new("lrr/v1/retained-local-function-readiness");
        identity.digest(self.identity());
        identity.digest(Digest(self.syntax.identity().0));
        identity.u32(body.start);
        identity.u32(body.end);
        Ok(identity.finish())
    }
}

impl SourceSyntaxAssembly {
    /// The first pending call, in tree order, lying anywhere inside `body`:
    /// the function's own calls and the calls of every function, closure,
    /// impl or trait member nested in it, macro-generated nested items
    /// included. A block-local function shares its enclosing body's retained
    /// tree (its identity, body handle and index are read from that text), so
    /// its calls attach in the same transaction as the enclosing body's own.
    /// A call with no declaration producer (cfg-disabled code) cannot be
    /// expanded: it stays a pending marker and blocks only the body that owns
    /// it directly (`owners` selects what owns a call), never an enclosing one.
    /// Calls are sorted by their current coordinates, so this is a binary
    /// search plus the calls inside the range; no snapshot is exported and no
    /// source is replayed.
    fn first_call_within(
        &self,
        body: SourceRange,
        owners: BodyOwners,
    ) -> Result<Option<usize>, Diagnostic> {
        let index = self.calls.partition_point(|call| call.start < body.start);
        for (offset, call) in self.calls[index..].iter().enumerate() {
            if call.start >= body.end {
                break;
            }
            if !body.contains(SourceRange::checked(call.start, call.end)?) {
                continue;
            }
            if call.cfg_disabled {
                // Stripped before expansion: nothing to expand or wait for.
                continue;
            }
            if call.invocation.is_none() {
                let owner = self
                    .tree
                    .call_owner_body(call.start, call.end, owners)
                    .map_err(adapter)?
                    .map(|(start, end)| SourceRange::checked(start, end))
                    .transpose()?;
                if owner != Some(body) {
                    continue;
                }
            }
            return Ok(Some(index + offset));
        }
        Ok(None)
    }
    fn binding_invocation(&self, index: usize) -> Result<(Digest, Digest), Diagnostic> {
        let binding = &self.calls[index];
        binding
            .invocation
            .map(|invocation| (binding.parent_identity, invocation))
            .ok_or_else(|| fault("selected body call lacks an actual declaration producer"))
    }

    /// Calls in nested functions belong to those functions, including generated
    /// functions that have no original declaration anchor. Resolve ownership
    /// from the actual retained tree after every structural edit.
    pub(crate) fn next_body_invocation(
        &self,
        definition: Digest,
    ) -> Result<Option<(Digest, Digest)>, Diagnostic> {
        let original = self
            .source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == definition)
            .ok_or_else(|| fault("selected assembly body is not an original declaration"))?;
        if original.def_key.item_kind != ItemKind::Function || original.body.is_none() {
            return Err(fault(
                "selected assembly body requires an original function",
            ));
        }
        let range = self
            .originals
            .iter()
            .find(|anchor| anchor.original_definition() == definition)
            .and_then(OriginalDeclarationAnchor::body_range)
            .ok_or_else(|| fault("selected assembly body anchor absent"))?;
        self.first_call_within(range, BodyOwners::Functions)?
            .map(|index| self.binding_invocation(index))
            .transpose()
    }
    /// A private scope tag is minted only after the exact translated function
    /// has no remaining function-owned calls. Other pending markers are kept.
    pub fn finish_for_body(self, definition: Digest) -> Result<PreparedSourceSyntax, Diagnostic> {
        if self.next_body_invocation(definition)?.is_some() {
            return Err(fault(
                "selected assembly body still has pending expansion calls",
            ));
        }
        self.finish_scope(Some(definition), None)
    }

    /// Current retained location of a macro-generated function: the exact
    /// first/last output positions of its declaration, projected through the
    /// actual structural splice map (never token origins or names). None until
    /// the producing record has been attached.
    fn locate_generated(
        &self,
        target: &GeneratedBodyTarget,
    ) -> Result<Option<(SourceRange, SourceRange)>, Diagnostic> {
        if target.output.len() < 2 {
            return Err(fault("generated function output range differs"));
        }
        let edge = |start: u32| -> Result<Option<SourceRange>, Diagnostic> {
            self.occurrences
                .project_generated(target.record, SourceRange::checked(start, start + 1)?)
        };
        let (Some(first), Some(last)) = (edge(target.output.start)?, edge(target.output.end - 1)?)
        else {
            return Ok(None);
        };
        let declaration = SourceRange::checked(first.start, last.end)?;
        let body = self
            .tree
            .declaration_body_at(declaration.start, declaration.end)
            .map_err(adapter)?
            .ok_or_else(|| fault("generated function retained declaration node absent"))?
            .ok_or_else(|| fault("generated function retained declaration has no body"))?;
        Ok(Some((declaration, SourceRange::checked(body.0, body.1)?)))
    }

    /// The selected generated function's current declaration/body location
    /// and the next pending call its own body owns (calls owned by nested
    /// functions stay pending; unrelated bodies are untouched).
    fn generated_progress(
        &self,
        target: &GeneratedBodyTarget,
    ) -> Result<(SourceRange, SourceRange, Option<(Digest, Digest)>), Diagnostic> {
        let (declaration, body) = self
            .locate_generated(target)?
            .ok_or_else(|| fault("selected generated function is not attached"))?;
        let next = self
            .first_call_within(body, BodyOwners::Entered)?
            .map(|index| self.binding_invocation(index))
            .transpose()?;
        Ok((declaration, body, next))
    }
    pub(crate) fn next_generated_body_invocation(
        &self,
        target: &GeneratedBodyTarget,
    ) -> Result<Option<(Digest, Digest)>, Diagnostic> {
        Ok(self.generated_progress(target)?.2)
    }

    /// Mint the selected-body certificate for a generated function whose own
    /// calls are all attached, with its exact retained structural anchor.
    pub(crate) fn finish_for_generated_body(
        self,
        target: &GeneratedBodyTarget,
    ) -> Result<PreparedSourceSyntax, Diagnostic> {
        let (declaration, body, next) = self.generated_progress(target)?;
        if next.is_some() {
            return Err(fault(
                "selected generated body still has pending expansion calls",
            ));
        }
        let anchor = super::GeneratedBodyAnchor {
            definition: target.definition,
            record: target.record,
            output: target.output,
            declaration,
            body,
        };
        self.finish_scope(Some(target.definition), Some(anchor))
    }
}

/// One demanded macro-generated function: its declaration identity, the key
/// fingerprint of the record that wrote it and its range in that output.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GeneratedBodyTarget {
    pub(crate) definition: Digest,
    pub(crate) record: Digest,
    pub(crate) output: SourceRange,
}
