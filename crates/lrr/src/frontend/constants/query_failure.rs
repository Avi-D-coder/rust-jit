//! Exact typed deterministic constant refusal. No guest values or authority.
use super::{ConstFailure, ConstPanic, query_identity::ConstQueryIdentity};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::dependency::InputKey,
    frontend::constant_source_admission::ConstantSourceAdmission,
    syntax::spans::SourceRange,
    vm::numeric::PanicKind,
};
#[derive(Debug, Clone)]
enum FailureKind {
    Type,
    Cycle,
    Numeric(PanicKind),
    Macro(ConstPanic),
}
#[derive(Debug, Clone)]
pub struct ConstantQueryFailure {
    key: ConstQueryIdentity,
    diagnostic: Diagnostic,
    source: Digest,
    range: SourceRange,
    kind: FailureKind,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-MEMO", message.into())
}
impl ConstantQueryFailure {
    /// Content binding only; caller retains/replays actual source admission.
    /// Resource/missing-input/unsupported errors cannot become deterministic memos.
    pub(crate) fn capture(
        key: &ConstQueryIdentity,
        admission: &ConstantSourceAdmission,
        failure: &ConstFailure,
    ) -> Result<Self, Diagnostic> {
        key.validate()?;
        let plan = admission.plan();
        if key.universe().is_none()
            || key.definition() != plan.owner_definition()
            || key.selection() != plan.identity()
            || key.source() != plan.source().source()
            || key.cfg() != plan.source().cfg()
            || key.profile() != plan.profile()
            || key.admission() != admission.identity()
            || failure.requested_plan() != Some(plan.identity())
        {
            return Err(invalid(
                "constant failure belongs to another requested current plan",
            ));
        }
        if !admission.dependencies().iter().any(|(input, _)| matches!(input, InputKey::SourceBlob(source) if *source == failure.source())) {
            return Err(invalid("constant failure origin is not an admitted current source"));
        }
        if failure.range().start > failure.range().end {
            return Err(invalid("constant failure source range differs"));
        }
        let diagnostic = failure.diagnostic();
        // Pure diagnostics have no live dispatcher, renderer or guest-stack state.
        if diagnostic.macro_origin.is_some()
            || diagnostic.rendered_macro_origin.is_some()
            || diagnostic.generated_origin.is_some()
            || diagnostic.rendered_generated_origin.is_some()
            || diagnostic.guest_stack.is_some()
            || diagnostic.rendered_guest_stack.is_some()
            || diagnostic.origin_attachment_failure.is_some()
            || diagnostic.prepared_input_change.is_some()
            || diagnostic.failure_context.is_some()
        {
            return Err(invalid(
                "constant pure failure carries execution diagnostic state",
            ));
        }
        diagnostic
            .heap_bytes()
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(key.metadata_bytes().ok()?))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>() + 128 + 2 * 65_536))            .ok_or_else(|| invalid("constant failure transient metadata limit"))?;
        let kind = match (diagnostic.code, failure.numeric_panic(), failure.panic()) {
            ("LRR-E-CONST-TYPE", None, None) => FailureKind::Type,
            ("LRR-E-CONST-CYCLE", None, None) => FailureKind::Cycle,
            (
                "LRR-E-CONST-PANIC",
                Some(
                    kind @ (PanicKind::Overflow
                    | PanicKind::DivisionByZero
                    | PanicKind::InvalidShift),
                ),
                None,
            ) if diagnostic.message == kind.description() => FailureKind::Numeric(kind),
            ("LRR-E-CONST-PANIC", None, Some(panic)) if diagnostic.message == panic.message() => {
                // Admit the clone before allocating either retained message.
                FailureKind::Macro(panic.clone())
            }
            _ => {
                return Err(invalid(
                    "constant failure is not a complete deterministic outcome",
                ));
            }
        };
        Ok(Self {
            key: key.clone(),
            diagnostic: diagnostic.clone(),
            source: failure.source(),
            range: failure.range(),
            kind,
        })
    }
    pub fn key(&self) -> &ConstQueryIdentity {
        &self.key
    }
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    pub fn source(&self) -> Digest {
        self.source
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    pub fn numeric_panic(&self) -> Option<PanicKind> {
        match self.kind {
            FailureKind::Numeric(kind) => Some(kind),
            _ => None,
        }
    }
    pub fn panic(&self) -> Option<&ConstPanic> {
        match &self.kind {
            FailureKind::Macro(panic) => Some(panic),
            _ => None,
        }
    }
    pub fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(128)
            .and_then(|bytes| bytes.checked_add(self.key.heap_bytes().ok()?))
            .and_then(|bytes| bytes.checked_add(self.diagnostic.heap_bytes()?))
            .and_then(|bytes| match &self.kind {
                FailureKind::Macro(panic) => bytes.checked_add(panic.message().len() + 128),
                _ => Some(bytes),
            })            .ok_or_else(|| invalid("constant failure metadata limit"))
    }
}
impl StableEncode for ConstantQueryFailure {
    const DOMAIN: &'static str = "lrr/v1/typed-constant-query-failure";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.string(self.diagnostic.code);
        e.string(&self.diagnostic.message);
        e.digest(self.source);
        e.u32(self.range.start);
        e.u32(self.range.end);
        match &self.kind {
            FailureKind::Type => e.u32(1),
            FailureKind::Cycle => e.u32(2),
            FailureKind::Numeric(kind) => {
                e.u32(3);
                e.u32(match kind {
                    PanicKind::Overflow => 1,
                    PanicKind::DivisionByZero => 2,
                    PanicKind::InvalidShift => 3,
                    // These stable tags are explicit even though capture excludes
                    // nonnumeric runtime panic kinds from this pure lane.
                    PanicKind::Explicit => 4,
                    PanicKind::Assertion => 5,
                    PanicKind::IndexOutOfBounds => 6,
                });
            }
            FailureKind::Macro(panic) => {
                e.u32(4);
                e.string(panic.message());
                e.digest(panic.definition());
                e.digest(panic.proof());
            }
        }
    }
}
