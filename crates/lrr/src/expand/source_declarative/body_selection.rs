//! Structural function ownership for pre-entry assembly demand. No name/type
//! inference, executable shape checking, expansion or guest mutation occurs here.
use super::*;
use crate::{source::manifest::ItemKind, syntax::parse::ParsedFile};

fn function_owns_call(
    parsed: &ParsedFile,
    body: SourceRange,
    call: &CallCapture,
) -> Result<bool, Diagnostic> {
    let call_range = SourceRange::checked(call.start, call.end)?;
    Ok(body.contains(call_range) && parsed.macro_call_function_body(call_range)? == body)
}

impl PreparedSourceSyntax {
    /// Cold structural closure proof for an actually selected local function.
    /// The caller must separately prove the declaration, source and parent
    /// authority. This does not broaden the assembly's scoped body permit.
    pub(crate) fn require_closed_function_body(
        &self,
        parsed: &ParsedFile,
        body: SourceRange,
    ) -> Result<Digest, Diagnostic> {
        if parsed.retained_syntax_identity() != Some(Digest(self.syntax.identity().0))
            || body.is_empty()
            || body.end as usize > self.syntax.syntax().text.len()
            || self.syntax.syntax().calls.len() > MAX_CALLS
        {
            return Err(fault("local readiness retained tree/body differs"));
        }
        for call in &self.syntax.syntax().calls {
            if function_owns_call(parsed, body, call)? {
                return Err(fault("local function still has pending expansion calls"));
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
        let snapshot = Arc::new(self.tree.snapshot().map_err(adapter)?);
        verify_calls(&self.calls, &snapshot.syntax().calls)?;
        let parsed = ParsedFile::from_syntax_snapshot(
            snapshot.clone(),
            self.source.selection().outline.parse.profile.clone(),
        )?;
        for (binding, call) in self.calls.iter().zip(&snapshot.syntax().calls) {
            if !function_owns_call(&parsed, range, call)? {
                continue; // A nested function is independently prepared at entry.
            }
            return binding
                .invocation
                .map(|invocation| Some((binding.parent_identity, invocation)))
                .ok_or_else(|| fault("selected body call lacks an actual declaration producer"));
        }
        Ok(None)
    }
    /// A private scope tag is minted only after the exact translated function
    /// has no remaining function-owned calls. Other pending markers are kept.
    pub fn finish_for_body(self, definition: Digest) -> Result<PreparedSourceSyntax, Diagnostic> {
        if self.next_body_invocation(definition)?.is_some() {
            return Err(fault(
                "selected assembly body still has pending expansion calls",
            ));
        }
        self.finish_scope(Some(definition))
    }
}
