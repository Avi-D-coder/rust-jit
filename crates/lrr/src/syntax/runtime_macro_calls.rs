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
        op::{BorrowAccess, HintRecipe, Op},
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
    PrimitiveMethod(Box<super::RuntimePrimitiveMethod>),
    CallPrepare(u32),
    CallAdvance(u32, u32),
    CallSeal(u32),
    Fault(Diagnostic),
    Control(Box<super::RuntimeScalarControl>),
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
    fault_origins: Vec<(PcKey, BodySpan)>,
}
impl RuntimeScalarPlan {
    pub(crate) fn steps(&self) -> &[RuntimeScalarStep] {
        &self.steps
    }
    pub(crate) fn fault_span(&self, key: &PcKey) -> Option<&BodySpan> {
        self.fault_origins
            .iter()
            .find_map(|(pc, span)| (pc == key).then_some(span))
    }
    pub(crate) fn call(&self, ordinal: u32) -> Result<&Arc<RuntimeMacroCall>, Diagnostic> {
        self.calls
            .get(ordinal as usize)
            .filter(|call| call.ordinal == ordinal)
            .ok_or_else(|| invalid("scalar call ordinal absent"))
    }
    /// A freshly replayed source plan selects only its exact borrow operation
    /// phases. The operand's arbitrary scalar operations gain no new authority.
    pub(crate) fn argument_borrow_at(
        &self,
        key: &PcKey,
    ) -> Result<Option<(&Arc<RuntimeMacroCall>, &RuntimeMacroArgumentBorrow)>, Diagnostic> {
        let mut selected = None;
        let mut work = 0usize;
        for call in &self.calls {
            for borrow in call.argument_borrows.iter().flatten() {
                work = work
                    .checked_add(1)
                    .filter(|n| *n <= 1_000_000)
                    .ok_or_else(|| invalid("scalar borrow lookup work limit"))?;
                if borrow.hint.as_ref() == Some(key)
                    || &borrow.place_key == key
                    || &borrow.borrow_key == key
                {
                    borrow.require(call, &key.body, borrow.argument)?;
                    if selected.replace((call, borrow)).is_some() {
                        return Err(invalid("scalar borrow phase selection is ambiguous"));
                    }
                }
            }
        }
        Ok(selected)
    }
    pub(super) fn finish(
        mut steps: Vec<RuntimeScalarStep>,
        drafts: Vec<CallDraft>,
        origins: Vec<(usize, BodySpan)>,
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
        retained_bytes = retained_bytes
            .checked_add(
                origins
                    .len()
                    .checked_mul(std::mem::size_of::<(PcKey, BodySpan)>())
                    .ok_or_else(|| invalid("scalar fault origin bytes overflow"))?,
            )
            .and_then(|n| {
                n.checked_add(
                    origins
                        .capacity()
                        .checked_mul(std::mem::size_of::<(usize, BodySpan)>())?,
                )
            })
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| invalid("scalar fault origin metadata limit"))?;
        let mut fault_origins = Vec::new();
        fault_origins
            .try_reserve_exact(origins.len())
            .map_err(|_| invalid("scalar fault PC allocation"))?;
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("scalar fault macro node absent"))?;
        let mut previous = None;
        for (at, span) in origins {
            if previous.is_some_and(|old| at <= old)
                || !matches!(steps.get(at), Some(RuntimeScalarStep::Fault(_)))
                || span.body_key != key.body
                || span.relative_start >= span.relative_end
                || span.relative_start < node.range.start
                || span.relative_end > node.range.end
            {
                return Err(invalid("scalar fault source phase/span differs"));
            }
            previous = Some(at);
            fault_origins.push((pc(at)?, span));
        }
        if fault_origins.len()
            != steps
                .iter()
                .filter(|step| matches!(step, RuntimeScalarStep::Fault(_)))
                .count()
        {
            return Err(invalid("scalar fault lacks exact source origin"));
        }
        for (ordinal, step) in steps.iter_mut().enumerate() {
            if let RuntimeScalarStep::PrimitiveMethod(method) = step {
                method.bind(index, key, ordinal)?;
                retained_bytes = retained_bytes
                    .checked_add(
                        method
                            .allocated_bytes()
                            .ok_or_else(|| invalid("primitive method metadata overflow"))?,
                    )
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| invalid("primitive method metadata limit"))?;
            }
        }
        for step in &steps {
            if let RuntimeScalarStep::Control(control) = step {
                retained_bytes = retained_bytes
                    .checked_add(control.allocated_bytes()?)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| invalid("scalar control plan metadata limit"))?;
            }
        }
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
            draft
                .argument_borrows
                .len()
                .checked_mul(std::mem::size_of::<Option<RuntimeMacroArgumentBorrow>>())
                .and_then(|n| {
                    draft
                        .argument_borrows
                        .iter()
                        .filter(|borrow| borrow.is_some())
                        .count()
                        .checked_mul(source.profile.semantics.capacity())
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| n.checked_add(retained_bytes))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| invalid("scalar borrow retained metadata limit"))?;
            let mut borrows = Vec::new();
            borrows
                .try_reserve_exact(draft.argument_borrows.len())
                .map_err(|_| invalid("scalar borrow certificate allocation"))?;
            for (ordinal, borrow) in draft.argument_borrows.into_iter().enumerate() {
                let Some(borrow) = borrow else {
                    borrows.push(None);
                    continue;
                };
                let argument = *draft
                    .arguments
                    .get(ordinal)
                    .ok_or_else(|| invalid("scalar borrow argument absent"))?;
                let argument_span = draft
                    .argument_spans
                    .get(ordinal)
                    .ok_or_else(|| invalid("scalar borrow argument origin absent"))?;
                if borrow.argument as usize != ordinal
                    || argument_span.body_key != borrow.borrow_span.body_key
                    || borrow.borrow_span.body_key != key.body
                    || argument_span.relative_start > borrow.borrow_span.relative_start
                    || argument_span.relative_end < borrow.borrow_span.relative_end
                    || borrow.operand_span.body_key != key.body
                    || borrow.borrow_span.relative_start >= borrow.operand_span.relative_start
                    || borrow.borrow_span.relative_end < borrow.operand_span.relative_end
                    || borrow.operand_span.relative_start >= borrow.operand_span.relative_end
                {
                    return Err(invalid("scalar borrow source containment differs"));
                }
                let mut hint = None;
                let mut place = None;
                let mut commit = None;
                for (at, step) in steps.iter().enumerate() {
                    work = work
                        .checked_add(1)
                        .filter(|n| *n <= 1_000_000)
                        .ok_or_else(|| invalid("scalar borrow phase capture work limit"))?;
                    if let RuntimeScalarStep::Operation(op) = step {
                        match op {
                            Op::SetHint {
                                child,
                                hint: HintRecipe::BorrowTarget { reference },
                            } if Some(*child) == borrow.temporary && *reference == argument => {
                                record_phase(&mut hint, at, "duplicate scalar borrow hint")?
                            }
                            Op::ValuePlace {
                                source,
                                destination,
                            } if Some(*source) == borrow.temporary
                                && *destination == borrow.place =>
                            {
                                record_phase(&mut place, at, "duplicate scalar temporary place")?
                            }
                            Op::MakePlace { destination, .. }
                            | Op::ParameterPlace { destination, .. }
                                if borrow.temporary.is_none() && *destination == borrow.place =>
                            {
                                record_phase(&mut place, at, "duplicate scalar local place")?
                            }
                            Op::Borrow {
                                place: actual,
                                access,
                                destination,
                            } if *actual == borrow.place
                                && *destination == argument
                                && *access == borrow.access =>
                            {
                                record_phase(&mut commit, at, "duplicate scalar borrow commit")?
                            }
                            _ => {}
                        }
                    }
                }
                let place = place.ok_or_else(|| invalid("scalar borrow place phase absent"))?;
                let commit = commit.ok_or_else(|| invalid("scalar borrow commit phase absent"))?;
                let begin = hint.unwrap_or(place);
                if begin <= prepare
                    || place.checked_add(1) != Some(commit)
                    || commit >= advances[ordinal]
                    || (borrow.temporary.is_some() && hint.is_none_or(|at| at >= place))
                    || (borrow.temporary.is_none() && hint.is_some())
                {
                    return Err(invalid("scalar borrow written phase order differs"));
                }
                borrows.push(Some(RuntimeMacroArgumentBorrow {
                    source: source.clone(),
                    call_ordinal: draft.ordinal,
                    argument: borrow.argument,
                    argument_span: argument_span.clone(),
                    borrow_span: borrow.borrow_span,
                    operand_span: borrow.operand_span,
                    access: borrow.access,
                    argument_slot: argument,
                    temporary: borrow.temporary,
                    place: borrow.place,
                    hint: hint.map(pc).transpose()?,
                    operand: hint
                        .map(|at| {
                            at.checked_add(1)
                                .ok_or_else(|| invalid("scalar borrow operand phase overflow"))
                                .and_then(pc)
                        })
                        .transpose()?,
                    place_key: pc(place)?,
                    borrow_key: pc(commit)?,
                }));
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
                argument_borrows: borrows,
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
            let mut e = StableEncoder::new("lrr/v2/runtime-macro-source-call/argument-borrows");
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
            e.sequence(&call.argument_borrows, |e, borrow| {
                e.option(borrow, |e, borrow| borrow.encode(e))
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
        Ok(Self {
            steps,
            calls,
            fault_origins,
        })
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
    pub(super) argument_borrows: Vec<Option<ArgumentBorrowDraft>>,
    pub(super) result: SlotId,
}
pub(super) struct ArgumentBorrowDraft {
    pub(super) argument: u32,
    pub(super) borrow_span: BodySpan,
    pub(super) operand_span: BodySpan,
    pub(super) access: BorrowAccess,
    pub(super) temporary: Option<SlotId>,
    pub(super) place: SlotId,
}
/// Original written call argument borrow; no pointee or region type authority.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RuntimeMacroArgumentBorrow {
    source: ParseKey,
    call_ordinal: u32,
    argument: u32,
    argument_span: BodySpan,
    borrow_span: BodySpan,
    operand_span: BodySpan,
    access: BorrowAccess,
    argument_slot: SlotId,
    temporary: Option<SlotId>,
    place: SlotId,
    hint: Option<PcKey>,
    operand: Option<PcKey>,
    place_key: PcKey,
    borrow_key: PcKey,
}
impl RuntimeMacroArgumentBorrow {
    pub(crate) fn source(&self) -> &ParseKey {
        &self.source
    }
    pub(crate) fn argument_ordinal(&self) -> u32 {
        self.argument
    }
    #[cfg(test)]
    pub(crate) fn argument_span(&self) -> &BodySpan {
        &self.argument_span
    }
    pub(crate) fn borrow_span(&self) -> &BodySpan {
        &self.borrow_span
    }
    #[cfg(test)]
    pub(crate) fn operand_span(&self) -> &BodySpan {
        &self.operand_span
    }
    pub(crate) fn access(&self) -> BorrowAccess {
        self.access
    }
    pub(crate) fn argument_slot(&self) -> SlotId {
        self.argument_slot
    }
    pub(crate) fn temporary(&self) -> Option<SlotId> {
        self.temporary
    }
    pub(crate) fn place(&self) -> SlotId {
        self.place
    }
    pub(crate) fn hint_key(&self) -> Option<&PcKey> {
        self.hint.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn operand_key(&self) -> Option<&PcKey> {
        self.operand.as_ref()
    }
    pub(crate) fn place_key(&self) -> &PcKey {
        &self.place_key
    }
    pub(crate) fn borrow_key(&self) -> &PcKey {
        &self.borrow_key
    }
    pub(crate) fn require(
        &self,
        call: &RuntimeMacroCall,
        body: &BodyKey,
        argument: u32,
    ) -> Result<(), Diagnostic> {
        if self.source != call.source
            || self.call_ordinal != call.ordinal
            || self.argument != argument
            || body != &call.body
            || self.argument_span.body_key != *body
            || call.arguments.get(argument as usize) != Some(&self.argument_slot)
            || call
                .argument_borrows
                .get(argument as usize)
                .and_then(Option::as_ref)
                != Some(self)
        {
            return Err(invalid("scalar argument borrow source/call/body differs"));
        }
        Ok(())
    }
    fn encode(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.u32(self.call_ordinal);
        e.u32(self.argument);
        for span in [&self.argument_span, &self.borrow_span, &self.operand_span] {
            e.record(&span.body_key);
            e.u32(span.relative_start);
            e.u32(span.relative_end);
        }
        e.u32(match self.access {
            BorrowAccess::Shared => 0,
            BorrowAccess::Mutable => 1,
        });
        e.u32(self.argument_slot.0);
        e.option(&self.temporary, |e, s| e.u32(s.0));
        e.u32(self.place.0);
        e.option(&self.hint, |e, k| e.record(k));
        e.option(&self.operand, |e, k| e.record(k));
        e.record(&self.place_key);
        e.record(&self.borrow_key);
    }
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
    argument_borrows: Vec<Option<RuntimeMacroArgumentBorrow>>,
    result: SlotId,
    prepare: PcKey,
    argument_keys: Vec<PcKey>,
    advance_keys: Vec<PcKey>,
    seal: PcKey,
    resume: PcKey,
    identity: Digest,
}
impl RuntimeMacroCall {
    pub(crate) fn argument_borrow(
        &self,
        argument: u32,
    ) -> Result<Option<&RuntimeMacroArgumentBorrow>, Diagnostic> {
        self.argument_borrows
            .get(argument as usize)
            .map(Option::as_ref)
            .ok_or_else(|| invalid("scalar borrow argument ordinal absent"))
    }
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
                self.argument_borrows.capacity(),
                std::mem::size_of::<Option<RuntimeMacroArgumentBorrow>>(),
            ),
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
        for borrow in self.argument_borrows.iter().flatten() {
            bytes = bytes
                .checked_add(borrow.source.profile.semantics.capacity())
                .ok_or_else(|| invalid("scalar borrow source metadata overflow"))?;
        }
        for text in &self.explicit {
            bytes = bytes
                .checked_add(text.capacity())
                .ok_or_else(|| invalid("scalar call metadata overflow"))?;
        }
        Ok(bytes)
    }
}
