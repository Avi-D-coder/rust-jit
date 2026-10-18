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
        let snapshot = self.tree.snapshot().map_err(adapter)?;
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
        self.finish_scope(Some(definition), None)
    }

    /// Current retained location of a macro-generated function: the exact
    /// first/last output positions of its declaration, projected through the
    /// actual structural splice map (never token origins or names). None until
    /// the producing record has been attached.
    fn locate_generated(
        &self,
        parsed: &ParsedFile,
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
        let node = parsed
            .syntax_at(declaration, |node| entered_body(node).is_some())?
            .ok_or_else(|| fault("generated function retained declaration node absent"))?;
        let body = entered_body(&node)
            .flatten()
            .ok_or_else(|| fault("generated function retained declaration has no body"))?;
        Ok(Some((declaration, body)))
    }

    /// The selected generated function's current declaration/body location
    /// and the next pending call its own body owns (calls owned by nested
    /// functions stay pending; unrelated bodies are untouched).
    fn generated_progress(
        &self,
        target: &GeneratedBodyTarget,
    ) -> Result<(SourceRange, SourceRange, Option<(Digest, Digest)>), Diagnostic> {
        let snapshot = self.tree.snapshot().map_err(adapter)?;
        verify_calls(&self.calls, &snapshot.syntax().calls)?;
        let parsed = ParsedFile::from_syntax_snapshot(
            snapshot.clone(),
            self.source.selection().outline.parse.profile.clone(),
        )?;
        let (declaration, body) = self
            .locate_generated(&parsed, target)?
            .ok_or_else(|| fault("selected generated function is not attached"))?;
        for (binding, call) in self.calls.iter().zip(&snapshot.syntax().calls) {
            if !entered_body_owns_call(&parsed, body, call)? {
                continue;
            }
            let next = binding
                .invocation
                .map(|invocation| (binding.parent_identity, invocation))
                .ok_or_else(|| fault("selected body call lacks an actual declaration producer"))?;
            return Ok((declaration, body, Some(next)));
        }
        Ok((declaration, body, None))
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

/// The entered body of a function, or the initializer of a constant/static
/// (Some(None) when the declaration has no body); None for other nodes.
fn entered_body(node: &ra_ap_syntax::SyntaxNode) -> Option<Option<SourceRange>> {
    use ra_ap_syntax::{AstNode, ast};
    let body = if let Some(function) = ast::Fn::cast(node.clone()) {
        function.body().map(|body| body.syntax().text_range())
    } else if let Some(constant) = ast::Const::cast(node.clone()) {
        constant.body().map(|body| body.syntax().text_range())
    } else if let Some(value) = ast::Static::cast(node.clone()) {
        value.body().map(|body| body.syntax().text_range())
    } else {
        return None;
    };
    Some(body.map(crate::syntax::ast_adapter::range))
}
/// Structural ownership: the innermost function/initializer around the call
/// is exactly `body`. Calls of nested functions belong to those functions.
fn entered_body_owns_call(
    parsed: &ParsedFile,
    body: SourceRange,
    call: &CallCapture,
) -> Result<bool, Diagnostic> {
    let call_range = SourceRange::checked(call.start, call.end)?;
    if !body.contains(call_range) {
        return Ok(false);
    }
    let syntax = parsed
        .syntax_at(call_range, |node| {
            <ra_ap_syntax::ast::MacroCall as ra_ap_syntax::AstNode>::can_cast(node.kind())
        })?
        .ok_or_else(|| fault("macro call syntax attachment absent"))?;
    Ok(syntax
        .ancestors()
        .find_map(|node| entered_body(&node).flatten())
        == Some(body))
}
/// One demanded macro-generated function: its declaration identity, the key
/// fingerprint of the record that wrote it and its range in that output.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GeneratedBodyTarget {
    pub(crate) definition: Digest,
    pub(crate) record: Digest,
    pub(crate) output: SourceRange,
}
