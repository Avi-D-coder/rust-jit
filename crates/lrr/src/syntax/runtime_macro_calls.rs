//! Source-owned calls inside an entered intrinsic's opaque operand tokens.
use super::super::{
    body_index::BodyIndex,
    parse::ParseKey,
    spans::{BodySpan, SourceRange},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    ir::{
        op::Op,
        pc::{NodeId, PcKey, Phase, SlotId},
    },
    source::manifest::BodyKey,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
use std::sync::Arc;

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-RUNTIME-MACRO-ARGUMENTS", message.into())
}
fn record_phase(target: &mut Option<usize>, at: usize, duplicate: &str) -> Result<(), Diagnostic> {
    if target.replace(at).is_some() {
        return Err(invalid(duplicate));
    }
    Ok(())
}
fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

/// The same direct path/explicit-argument extraction contract as ordinary
/// entered calls. Selection and lexical authority are deliberately not granted.
pub(super) fn source_call_path(call: &ast::CallExpr) -> Result<(String, Vec<String>), Diagnostic> {
    let Some(ast::Expr::PathExpr(path)) = call.expr() else {
        return Err(unsupported(
            "assertion callee values require function-value integration",
        ));
    };
    let path = path
        .path()
        .ok_or_else(|| unsupported("assertion callee path absent"))?;
    let segment = path
        .segment()
        .ok_or_else(|| unsupported("assertion callee segment absent"))?;
    if segment.name_ref().is_none() {
        return Err(unsupported("assertion callee name absent"));
    }
    let mut components = Vec::new();
    let mut cursor = path;
    let absolute;
    loop {
        if components.len() >= 256 {
            return Err(invalid("assertion call path depth limit"));
        }
        let part = cursor
            .segment()
            .ok_or_else(|| unsupported("assertion call segment absent"))?;
        if !components.is_empty() && part.generic_arg_list().is_some() {
            return Err(unsupported(
                "generic assertion path owner requires associated-call integration",
            ));
        }
        let text = match part.kind() {
            Some(ast::PathSegmentKind::Name(n)) => n.text().to_string(),
            Some(ast::PathSegmentKind::CrateKw) => "crate".into(),
            Some(ast::PathSegmentKind::SelfKw) => "self".into(),
            Some(ast::PathSegmentKind::SuperKw) => "super".into(),
            _ => {
                return Err(unsupported(
                    "assertion call path requires associated-call integration",
                ));
            }
        };
        components
            .try_reserve(1)
            .map_err(|_| invalid("assertion call path allocation"))?;
        components.push(text);
        if let Some(parent) = cursor.qualifier() {
            cursor = parent;
        } else {
            absolute = part.coloncolon_token().is_some();
            break;
        }
    }
    components.reverse();
    let mut name = components.join("::");
    if absolute {
        name.insert_str(0, "::");
    }
    if name.len() > 4096 {
        return Err(invalid("assertion call path spelling limit"));
    }
    let mut explicit = Vec::new();
    let mut bytes = 0usize;
    if let Some(list) = segment.generic_arg_list() {
        for arg in list.generic_args() {
            if explicit.len() >= 256 {
                return Err(invalid("assertion explicit argument count limit"));
            }
            let text = arg.syntax().text().to_string();
            bytes = bytes
                .checked_add(text.len())
                .filter(|n| *n <= 65_500)
                .ok_or_else(|| invalid("assertion explicit argument text limit"))?;
            explicit
                .try_reserve(1)
                .map_err(|_| invalid("assertion explicit argument allocation"))?;
            explicit.push(text);
        }
    }
    Ok((name, explicit))
}

#[derive(Debug)]
pub(crate) enum RuntimeScalarStep {
    Operation(Op),
    CallPrepare(u32),
    CallAdvance(u32, u32),
    CallSeal(u32),
    Fault(Diagnostic),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RuntimeMacroCallPhase {
    Prepare,
    Advance(u32),
    Seal,
}
#[derive(Debug)]
pub(crate) struct RuntimeScalarPlan {
    steps: Vec<RuntimeScalarStep>,
    calls: Vec<Arc<RuntimeMacroCall>>,
}
impl RuntimeScalarPlan {
    pub(crate) fn steps(&self) -> &[RuntimeScalarStep] {
        &self.steps
    }
    pub(crate) fn call(&self, ordinal: u32) -> Result<&Arc<RuntimeMacroCall>, Diagnostic> {
        self.calls
            .get(ordinal as usize)
            .filter(|call| call.ordinal == ordinal)
            .ok_or_else(|| invalid("scalar call ordinal absent"))
    }
    pub(super) fn finish(
        steps: Vec<RuntimeScalarStep>,
        drafts: Vec<CallDraft>,
        index: &BodyIndex,
        key: &PcKey,
        source: ParseKey,
        invocation: SourceRange,
    ) -> Result<Self, Diagnostic> {
        let pc = |ordinal: usize| -> Result<PcKey, Diagnostic> {
            let phase = if ordinal == 0 {
                Phase::Entry
            } else {
                Phase::Operand(
                    u16::try_from(ordinal - 1)
                        .map_err(|_| invalid("scalar call phase overflow"))?,
                )
            };
            index.pc(NodeId(key.node), phase)
        };
        let mut calls: Vec<Arc<RuntimeMacroCall>> = Vec::new();
        calls
            .try_reserve_exact(drafts.len())
            .map_err(|_| invalid("scalar call table allocation"))?;
        let mut retained_bytes = std::mem::size_of::<Self>()
            .checked_add(
                steps
                    .capacity()
                    .checked_mul(std::mem::size_of::<RuntimeScalarStep>())
                    .ok_or_else(|| invalid("scalar plan metadata overflow"))?,
            )
            .and_then(|n| {
                n.checked_add(
                    calls
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<RuntimeMacroCall>>())?,
                )
            })
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| invalid("scalar plan metadata limit"))?;
        let mut work = 0usize;
        for draft in drafts {
            let mut prepare = None;
            let mut seal = None;
            let mut advances = Vec::new();
            advances
                .try_reserve_exact(draft.arguments.len())
                .map_err(|_| invalid("scalar call phase allocation"))?;
            for (at, step) in steps.iter().enumerate() {
                work = work
                    .checked_add(1)
                    .filter(|n| *n <= 1_000_000)
                    .ok_or_else(|| invalid("scalar call phase capture work limit"))?;
                match step {
                    RuntimeScalarStep::CallPrepare(id) if *id == draft.ordinal => {
                        record_phase(&mut prepare, at, "duplicate scalar call prepare")?;
                    }
                    RuntimeScalarStep::CallAdvance(id, arg) if *id == draft.ordinal => {
                        if usize::try_from(*arg).ok() != Some(advances.len()) {
                            return Err(invalid("scalar call written argument order differs"));
                        }
                        advances.push(at);
                    }
                    RuntimeScalarStep::CallSeal(id) if *id == draft.ordinal => {
                        record_phase(&mut seal, at, "duplicate scalar call seal")?;
                    }
                    _ => {}
                }
            }
            let prepare = prepare.ok_or_else(|| invalid("scalar call prepare absent"))?;
            let seal = seal.ok_or_else(|| invalid("scalar call seal absent"))?;
            let resume = seal
                .checked_add(1)
                .ok_or_else(|| invalid("scalar call resume overflow"))?;
            if advances.len() != draft.arguments.len() || prepare >= seal || resume >= steps.len() {
                return Err(invalid("scalar call phase boundary differs"));
            }
            let mut argument_keys = Vec::new();
            let mut advance_keys = Vec::new();
            argument_keys
                .try_reserve_exact(advances.len())
                .map_err(|_| invalid("scalar argument PC allocation"))?;
            advance_keys
                .try_reserve_exact(advances.len())
                .map_err(|_| invalid("scalar advance PC allocation"))?;
            for (arg, at) in advances.iter().copied().enumerate() {
                argument_keys.push(pc(if arg == 0 {
                    prepare
                        .checked_add(1)
                        .ok_or_else(|| invalid("scalar argument phase overflow"))?
                } else {
                    advances[arg - 1]
                        .checked_add(1)
                        .ok_or_else(|| invalid("scalar argument phase overflow"))?
                })?);
                advance_keys.push(pc(at)?);
            }
            let seal_at = if let Some(last) = advances.last() {
                if last.checked_add(1) != Some(seal) {
                    return Err(invalid("scalar final advance does not precede seal"));
                }
                *last
            } else {
                seal
            };
            if draft.parent.is_some_and(|parent| parent >= draft.ordinal)
                || usize::try_from(draft.ordinal).ok() != Some(calls.len())
            {
                return Err(invalid("scalar call parent/order differs"));
            }
            if let Some(parent) = draft.parent {
                let parent = calls
                    .get(parent as usize)
                    .ok_or_else(|| invalid("scalar call parent absent"))?;
                if !parent.argument_spans.iter().any(|span| {
                    span.body_key == draft.span.body_key
                        && span.relative_start <= draft.span.relative_start
                        && span.relative_end >= draft.span.relative_end
                }) {
                    return Err(invalid(
                        "scalar nested call outside written parent arguments",
                    ));
                }
            }
            let mut call = RuntimeMacroCall {
                source: source.clone(),
                body: key.body.clone(),
                macro_node: key.node,
                invocation,
                ordinal: draft.ordinal,
                parent: draft.parent,
                span: draft.span,
                name: draft.name,
                explicit: draft.explicit,
                arguments: draft.arguments,
                argument_spans: draft.argument_spans,
                result: draft.result,
                prepare: pc(prepare)?,
                argument_keys,
                advance_keys,
                seal: pc(seal_at)?,
                resume: pc(resume)?,
                identity: Digest([0; 32]),
            };
            let bytes = call.allocated_bytes()?;
            retained_bytes = retained_bytes
                .checked_add(bytes)
                .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| invalid("scalar call retained metadata limit"))?;
            let mut e = StableEncoder::new("lrr/v1/runtime-macro-source-call");
            e.record(&call.source);
            e.record(&call.body);
            e.u32(call.macro_node);
            e.u32(call.invocation.start);
            e.u32(call.invocation.end);
            e.u32(call.ordinal);
            e.option(&call.parent, |e, n| e.u32(*n));
            e.u32(call.span.relative_start);
            e.u32(call.span.relative_end);
            e.string(&call.name);
            e.sequence(&call.explicit, |e, t| e.string(t));
            e.sequence(&call.arguments, |e, s| e.u32(s.0));
            e.sequence(&call.argument_spans, |e, span| {
                e.u32(span.relative_start);
                e.u32(span.relative_end);
            });
            e.u32(call.result.0);
            e.record(&call.prepare);
            e.sequence(&call.argument_keys, |e, k| e.record(k));
            e.sequence(&call.advance_keys, |e, k| e.record(k));
            e.record(&call.seal);
            e.record(&call.resume);
            call.identity = e.finish();
            calls.push(Arc::new(call));
        }
        Ok(Self { steps, calls })
    }
}

pub(super) struct CallDraft {
    pub(super) ordinal: u32,
    pub(super) parent: Option<u32>,
    pub(super) span: BodySpan,
    pub(super) name: String,
    pub(super) explicit: Vec<String>,
    pub(super) arguments: Vec<SlotId>,
    pub(super) argument_spans: Vec<BodySpan>,
    pub(super) result: SlotId,
}
#[derive(Debug)]
pub(crate) struct RuntimeMacroCall {
    source: ParseKey,
    body: BodyKey,
    macro_node: u32,
    invocation: SourceRange,
    ordinal: u32,
    parent: Option<u32>,
    span: BodySpan,
    name: String,
    explicit: Vec<String>,
    arguments: Vec<SlotId>,
    argument_spans: Vec<BodySpan>,
    result: SlotId,
    prepare: PcKey,
    argument_keys: Vec<PcKey>,
    advance_keys: Vec<PcKey>,
    seal: PcKey,
    resume: PcKey,
    identity: Digest,
}
impl RuntimeMacroCall {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn parent(&self) -> Option<u32> {
        self.parent
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn explicit(&self) -> &[String] {
        &self.explicit
    }
    pub(crate) fn arguments(&self) -> &[SlotId] {
        &self.arguments
    }
    pub(crate) fn result(&self) -> SlotId {
        self.result
    }
    pub(crate) fn source(&self) -> &ParseKey {
        &self.source
    }
    #[cfg(test)]
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.span
    }
    pub(crate) fn invocation(&self) -> SourceRange {
        self.invocation
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn prepare_key(&self) -> &PcKey {
        &self.prepare
    }
    pub(crate) fn argument_key(&self, arg: u32) -> Result<&PcKey, Diagnostic> {
        self.argument_keys
            .get(arg as usize)
            .ok_or_else(|| invalid("scalar argument PC absent"))
    }
    pub(crate) fn advance_key(&self, arg: u32) -> Result<&PcKey, Diagnostic> {
        self.advance_keys
            .get(arg as usize)
            .ok_or_else(|| invalid("scalar advance PC absent"))
    }
    pub(crate) fn seal_key(&self) -> &PcKey {
        &self.seal
    }
    pub(crate) fn resume_key(&self) -> &PcKey {
        &self.resume
    }
    pub(crate) fn require(
        &self,
        body: &BodyKey,
        key: &PcKey,
        phase: RuntimeMacroCallPhase,
    ) -> Result<(), Diagnostic> {
        let expected = match phase {
            RuntimeMacroCallPhase::Prepare => &self.prepare,
            RuntimeMacroCallPhase::Advance(arg) => self.advance_key(arg)?,
            RuntimeMacroCallPhase::Seal => &self.seal,
        };
        if body != &self.body
            || key.body != self.body
            || key.node != self.macro_node
            || key != expected
        {
            return Err(invalid("source scalar call body/PC/phase differs"));
        }
        Ok(())
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(self.source.profile.semantics.capacity()))
            .and_then(|n| n.checked_add(self.name.capacity()))
            .ok_or_else(|| invalid("scalar call metadata overflow"))?;
        for (count, size) in [
            (self.explicit.capacity(), std::mem::size_of::<String>()),
            (self.arguments.capacity(), std::mem::size_of::<SlotId>()),
            (
                self.argument_spans.capacity(),
                std::mem::size_of::<BodySpan>(),
            ),
            (self.argument_keys.capacity(), std::mem::size_of::<PcKey>()),
            (self.advance_keys.capacity(), std::mem::size_of::<PcKey>()),
        ] {
            bytes = bytes
                .checked_add(
                    count
                        .checked_mul(size)
                        .ok_or_else(|| invalid("scalar call metadata overflow"))?,
                )
                .ok_or_else(|| invalid("scalar call metadata overflow"))?;
        }
        for text in &self.explicit {
            bytes = bytes
                .checked_add(text.capacity())
                .ok_or_else(|| invalid("scalar call metadata overflow"))?;
        }
        Ok(bytes)
    }
}
