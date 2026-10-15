//! Structural function ownership for pre-entry assembly demand. No name/type
//! inference, executable shape checking, expansion or guest mutation occurs here.
use super::*;
use crate::{source::manifest::ItemKind, syntax::parse::ParsedFile};

impl SourceSyntaxAssembly {
    /// Calls in nested functions belong to those functions, including generated
    /// functions that have no original declaration anchor. Resolve ownership
    /// from the actual retained tree after every structural edit.
    pub(crate) fn next_body_invocation(
        &self,
        definition: Digest,
    ) -> Result<Option<(Digest, Option<Digest>)>, Diagnostic> {
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
            let call_range = SourceRange::checked(call.start, call.end)?;
            if !range.contains(call_range) {
                continue;
            }
            let owner = parsed.macro_call_function_body(call_range)?;
            if owner != range {
                continue; // A nested function is independently prepared at entry.
            }
            return binding
                .invocation
                .map(|invocation| Some((invocation, binding.original_item_invocation)))
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
