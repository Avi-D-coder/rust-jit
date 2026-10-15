//! Source-owned forward branches inside the existing intrinsic operand phases.
//! No new body, guest evaluator, namespace authority or macro permission.
use super::*;
use crate::{
    core::digest::{Digest, StableEncode, StableEncoder},
    ir::{
        pc::{NodeId, Phase},
        recipe::RecipeExit,
    },
    syntax::{parse::ParseKey, spans::SourceRange},
};
use ra_ap_syntax::ast::{RangeItem, RangeOp};
#[derive(Clone, Copy)]
pub(super) struct Context {
    pub(super) destination: SlotId,
    pub(super) base: u32,
    pub(super) parent: Option<u32>,
}
#[derive(Clone, Copy)]
pub(super) struct Label(usize);
enum DraftExit {
    Exhausted(ExhaustionSite),
    Next(Label),
    Branch {
        condition: SlotId,
        yes: Label,
        no: Label,
    },
}
pub(super) struct Draft {
    span: BodySpan,
    exit: DraftExit,
}
/// Captured only while lowering a genuine, byte-matched original MatchExpr.
/// It is not a name/type/exhaustiveness judgement; actual arm tests must fail
/// before this source phase becomes reachable.
#[derive(Clone, Debug)]
struct ExhaustionSite {
    operand: BodySpan,
    scrutinee: BodySpan,
    source: SlotId,
    arms: Vec<BodySpan>,
}
#[derive(Debug)]
enum SelectedExit {
    Exhausted(ExhaustionSite),
    Next(PcKey),
    Branch {
        condition: SlotId,
        yes: PcKey,
        no: PcKey,
    },
}
#[derive(Debug)]
pub(crate) struct RuntimeScalarControl {
    source: ParseKey,
    body_index: Digest,
    invocation: SourceRange,
    body_start: u32,
    span: BodySpan,
    key: PcKey,
    exit: SelectedExit,
    identity: Digest,
}
/// Borrowed, opaque permission for exactly one source-owned Match exhaustion
/// recipe. Ordinary Control/Fault values cannot construct this witness.
pub(crate) struct RuntimeScalarMatchExhaustion<'a> {
    control: &'a RuntimeScalarControl,
}
impl RuntimeScalarMatchExhaustion<'_> {
    pub(crate) fn require(&self, index: &BodyIndex, key: &PcKey) -> Result<(), Diagnostic> {
        self.control.require(&self.control.source, index, key)?;
        if !matches!(self.control.exit, SelectedExit::Exhausted(_)) {
            return Err(invalid("scalar match exhaustion lacks actual source site"));
        }
        Ok(())
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.control.span
    }
}
impl RuntimeScalarControl {
    pub(crate) fn require(
        &self,
        source: &ParseKey,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<(), Diagnostic> {
        self.require_index(source, index, key, index.fingerprint())
    }
    fn require_index(
        &self,
        source: &ParseKey,
        index: &BodyIndex,
        key: &PcKey,
        fingerprint: Digest,
    ) -> Result<(), Diagnostic> {
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("scalar control node absent"))?;
        if self.identity != self.fingerprint()
            || &self.source != source
            || &self.key != key
            || index.body() != &key.body
            || fingerprint != self.body_index
            || index.profile() != &source.profile
            || node.role != NodeRole::Expression(ExpressionKind::Macro)
            || node.range.start.checked_add(self.body_start) != Some(self.invocation.start)
            || node.range.end.checked_add(self.body_start) != Some(self.invocation.end)
            || self.span.body_key != key.body
            || self.span.relative_start >= self.span.relative_end
            || self.span.relative_start < node.range.start
            || self.span.relative_end > node.range.end
        {
            return Err(invalid("scalar control source/body/phase/origin differs"));
        }
        let ordinal = step_ordinal(key)?;
        let target = |next: &PcKey| -> Result<(), Diagnostic> {
            if next.body != key.body || next.node != key.node || step_ordinal(next)? <= ordinal {
                return Err(invalid(
                    "scalar control target is not a forward source phase",
                ));
            }
            index.pc(NodeId(next.node), Phase::from_code(next.phase))?;
            Ok(())
        };
        index.pc(NodeId(key.node), Phase::from_code(key.phase))?;
        match &self.exit {
            SelectedExit::Exhausted(site) => {
                let inside = |span: &BodySpan| {
                    span.body_key == key.body
                        && span.relative_start >= self.span.relative_start
                        && span.relative_start < span.relative_end
                        && span.relative_end <= self.span.relative_end
                };
                if site.operand.body_key != key.body
                    || site.operand.relative_start < node.range.start
                    || site.operand.relative_end > node.range.end
                    || site.operand.relative_start > self.span.relative_start
                    || site.operand.relative_end < self.span.relative_end
                    || !node.runtime_scalar_slots.contains(&site.source)
                    || !inside(&site.scrutinee)
                    || site.arms.len() > 4096
                    || site.arms.iter().any(|arm| !inside(arm))
                    || site
                        .arms
                        .first()
                        .is_some_and(|arm| site.scrutinee.relative_end > arm.relative_start)
                    || site
                        .arms
                        .windows(2)
                        .any(|pair| pair[0].relative_end > pair[1].relative_start)
                {
                    return Err(invalid("scalar match exhaustion source structure differs"));
                }
            }
            SelectedExit::Next(next) => target(next)?,
            SelectedExit::Branch { condition, yes, no } => {
                if !node.runtime_scalar_slots.contains(condition) {
                    return Err(invalid("scalar control condition not source-reserved"));
                }
                target(yes)?;
                target(no)?;
            }
        }
        Ok(())
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.span
    }
    pub(crate) fn exhaustion(&self) -> Option<RuntimeScalarMatchExhaustion<'_>> {
        matches!(self.exit, SelectedExit::Exhausted(_))
            .then_some(RuntimeScalarMatchExhaustion { control: self })
    }
    pub(crate) fn recipe_exit(&self) -> RecipeExit {
        match &self.exit {
            SelectedExit::Exhausted(_) => RecipeExit::MatchNoArm,
            SelectedExit::Next(next) => RecipeExit::Next(next.clone()),
            SelectedExit::Branch { condition, yes, no } => RecipeExit::Branch {
                condition: *condition,
                yes: yes.clone(),
                no: no.clone(),
            },
        }
    }
    pub(super) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.source.profile.semantics.capacity())
            .and_then(|n| match &self.exit {
                SelectedExit::Exhausted(site) => n.checked_add(
                    site.arms
                        .capacity()
                        .checked_mul(std::mem::size_of::<BodySpan>())?,
                ),
                _ => Some(n),
            })
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| invalid("scalar control metadata limit"))
    }
}
impl StableEncode for RuntimeScalarControl {
    const DOMAIN: &'static str = "lrr/v1/runtime-scalar-source-control";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.digest(self.body_index);
        e.u32(self.invocation.start);
        e.u32(self.invocation.end);
        e.u32(self.body_start);
        e.record(&self.span.body_key);
        e.u32(self.span.relative_start);
        e.u32(self.span.relative_end);
        e.record(&self.key);
        match &self.exit {
            SelectedExit::Exhausted(site) => {
                e.u8(2);
                e.u32(site.source.0);
                e.record(&site.operand.body_key);
                e.u32(site.operand.relative_start);
                e.u32(site.operand.relative_end);
                e.record(&site.scrutinee.body_key);
                e.u32(site.scrutinee.relative_start);
                e.u32(site.scrutinee.relative_end);
                e.u64(site.arms.len() as u64);
                for arm in &site.arms {
                    e.record(&arm.body_key);
                    e.u32(arm.relative_start);
                    e.u32(arm.relative_end);
                }
            }
            SelectedExit::Next(next) => {
                e.u8(0);
                e.record(next);
            }
            SelectedExit::Branch { condition, yes, no } => {
                e.u8(1);
                e.u32(condition.0);
                e.record(yes);
                e.record(no);
            }
        }
    }
}
fn step_ordinal(key: &PcKey) -> Result<usize, Diagnostic> {
    match Phase::from_code(key.phase) {
        Phase::Entry => Ok(0),
        Phase::Operand(n) => Ok(usize::from(n) + 1),
        _ => Err(invalid("scalar control nonoperand phase")),
    }
}
pub(super) struct Builder {
    labels: Vec<Option<usize>>,
    patches: Vec<(usize, Draft)>,
    limit: usize,
}
impl Builder {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            labels: vec![],
            patches: vec![],
            limit,
        }
    }
    fn label(&mut self) -> Result<Label, Diagnostic> {
        if self.labels.len() >= self.limit {
            return Err(invalid("scalar control label reservation exhausted"));
        }
        self.labels
            .try_reserve(1)
            .map_err(|_| invalid("scalar control label allocation"))?;
        let label = Label(self.labels.len());
        self.labels.push(None);
        Ok(label)
    }
    pub(super) fn mark(&mut self, label: Label, at: usize) -> Result<(), Diagnostic> {
        let row = self
            .labels
            .get_mut(label.0)
            .ok_or_else(|| invalid("scalar control label absent"))?;
        if row.replace(at).is_some() {
            return Err(invalid("scalar control label repeated"));
        }
        Ok(())
    }
    pub(super) fn emit(
        &mut self,
        draft: Draft,
        steps: &mut Vec<RuntimeScalarStep>,
    ) -> Result<(), Diagnostic> {
        if self.patches.len() >= self.limit {
            return Err(invalid("scalar control branch reservation exhausted"));
        }
        self.patches
            .try_reserve(1)
            .map_err(|_| invalid("scalar control branch allocation"))?;
        self.patches.push((steps.len(), draft));
        steps.push(RuntimeScalarStep::Operation(Op::Declaration));
        Ok(())
    }
    pub(super) fn needs_tail(&self, len: usize) -> bool {
        self.labels.contains(&Some(len))
    }
    pub(super) fn finish(
        self,
        steps: &mut Vec<RuntimeScalarStep>,
        index: &BodyIndex,
        key: &PcKey,
        source: ParseKey,
        origin: (SourceRange, u32),
    ) -> Result<(), Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.labels
                    .capacity()
                    .checked_mul(std::mem::size_of::<Option<usize>>())
                    .ok_or_else(|| invalid("scalar control label bytes overflow"))?,
            )
            .and_then(|n| {
                n.checked_add(
                    self.patches
                        .capacity()
                        .checked_mul(std::mem::size_of::<(usize, Draft)>())?,
                )
            })
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| invalid("scalar control builder metadata limit"))?;
        for (_, draft) in &self.patches {
            if let DraftExit::Exhausted(site) = &draft.exit {
                bytes = bytes
                    .checked_add(
                        site.arms
                            .capacity()
                            .checked_mul(std::mem::size_of::<BodySpan>())
                            .ok_or_else(|| invalid("scalar match arm metadata overflow"))?,
                    )
                    .filter(|n| *n <= 32 << 20)
                    .ok_or_else(|| invalid("scalar match aggregate metadata limit"))?;
            }
        }
        let count = steps.len();
        bytes
            .checked_add(
                self.patches
                    .len()
                    .checked_mul(
                        std::mem::size_of::<RuntimeScalarControl>()
                            .checked_add(source.profile.semantics.capacity())
                            .ok_or_else(|| invalid("scalar control proof bytes overflow"))?,
                    )
                    .ok_or_else(|| invalid("scalar control proof count overflow"))?,
            )
            .and_then(|n| {
                n.checked_add(
                    steps
                        .capacity()
                        .checked_mul(std::mem::size_of::<RuntimeScalarStep>())?,
                )
            })
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| invalid("scalar control aggregate metadata limit"))?;
        let body_index = index.fingerprint();
        let pc = |at: usize| -> Result<PcKey, Diagnostic> {
            if at >= count {
                return Err(invalid("scalar control target outside plan"));
            }
            index.pc(
                NodeId(key.node),
                if at == 0 {
                    Phase::Entry
                } else {
                    Phase::Operand(
                        u16::try_from(at - 1)
                            .map_err(|_| invalid("scalar control phase overflow"))?,
                    )
                },
            )
        };
        for (at, draft) in self.patches {
            let resolve = |label: Label| -> Result<PcKey, Diagnostic> {
                pc(self
                    .labels
                    .get(label.0)
                    .and_then(|p| *p)
                    .ok_or_else(|| invalid("scalar control unresolved target"))?)
            };
            let exit = match draft.exit {
                DraftExit::Exhausted(site) => SelectedExit::Exhausted(site),
                DraftExit::Next(next) => SelectedExit::Next(resolve(next)?),
                DraftExit::Branch { condition, yes, no } => SelectedExit::Branch {
                    condition,
                    yes: resolve(yes)?,
                    no: resolve(no)?,
                },
            };
            let mut proof = RuntimeScalarControl {
                source: source.clone(),
                body_index,
                invocation: origin.0,
                body_start: origin.1,
                span: draft.span,
                key: pc(at)?,
                exit,
                identity: Digest([0; 32]),
            };
            proof.identity = proof.fingerprint();
            proof.require_index(&source, index, &proof.key, body_index)?;
            bytes = bytes
                .checked_add(proof.allocated_bytes()?)
                .filter(|n| *n <= 32 << 20)
                .ok_or_else(|| invalid("scalar control retained metadata limit"))?;
            steps[at] = RuntimeScalarStep::Control(Box::new(proof));
        }
        Ok(())
    }
    fn jump(span: &BodySpan, next: Label) -> Work {
        Work::Control(Draft {
            span: span.clone(),
            exit: DraftExit::Next(next),
        })
    }
    fn branch(span: &BodySpan, condition: SlotId, yes: Label, no: Label) -> Work {
        Work::Control(Draft {
            span: span.clone(),
            exit: DraftExit::Branch { condition, yes, no },
        })
    }
    pub(super) fn match_expression(
        &mut self,
        expression: ast::MatchExpr,
        context: Context,
        allocate: &mut impl FnMut() -> Result<SlotId, Diagnostic>,
        stack: &mut Vec<Work>,
        span: &impl Fn(&ast::Expr) -> Result<BodySpan, Diagnostic>,
    ) -> Result<(), Diagnostic> {
        let origin = span(&ast::Expr::MatchExpr(expression.clone()))?;
        // This ancestor is the existing actual operand decoder's initializer;
        // its original-byte offset was retained by Context, not invented here.
        let operand = expression
            .syntax()
            .ancestors()
            .find_map(ast::LetStmt::cast)
            .and_then(|statement| statement.initializer())
            .ok_or_else(|| invalid("scalar match original operand owner absent"))?;
        let operand_span = span(&operand)?;
        let source = allocate()?;
        let scrutinee = expression.expr().ok_or_else(unsupported)?;
        let scrutinee_span = span(&scrutinee)?;
        let result = self.label()?;
        let mut arms = Vec::new();
        for arm in expression.match_arm_list().ok_or_else(unsupported)?.arms() {
            if arms.len() >= 4096 {
                return Err(invalid("scalar match arm limit"));
            }
            arms.try_reserve(1)
                .map_err(|_| invalid("scalar match arm allocation"))?;
            arms.push((arm, self.label()?));
        }
        let end = self.label()?;
        let mut sequence = self.sequence(
            arms.len()
                .checked_mul(32)
                .and_then(|n| n.checked_add(4))
                .ok_or_else(|| invalid("scalar match work overflow"))?,
        )?;
        sequence.push(Work::Expression(
            scrutinee,
            source,
            context.base,
            context.parent,
        ));
        for (ordinal, (arm, entry)) in arms.iter().enumerate() {
            sequence.push(Work::Mark(*entry));
            let yes = self.label()?;
            let no = arms.get(ordinal + 1).map_or(end, |(_, entry)| *entry);
            let pattern = arm.pat().ok_or_else(unsupported)?;
            let guarded = arm.guard().is_some()
                || arm
                    .syntax()
                    .children()
                    .any(|child| ast::Attr::can_cast(child.kind()));
            self.pattern(
                Pattern {
                    pattern,
                    source,
                    yes,
                    no,
                    origin: &origin,
                },
                context,
                allocate,
                &mut sequence,
                span,
            )?;
            sequence.push(Work::Mark(yes));
            if guarded {
                sequence.push(Work::Fault(
                    unsupported(),
                    map_syntax(
                        arm.guard()
                            .map(|g| g.syntax().clone())
                            .as_ref()
                            .unwrap_or(arm.syntax()),
                        context,
                        &origin,
                    )?,
                ));
            } else {
                sequence.push(Work::Expression(
                    arm.expr().ok_or_else(unsupported)?,
                    context.destination,
                    context.base,
                    context.parent,
                ));
            }
            sequence.push(Work::Operation(Op::Discard { source }));
            sequence.push(Self::jump(&origin, result));
        }
        sequence.push(Work::Mark(end));
        arms.len()
            .checked_mul(std::mem::size_of::<BodySpan>())
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(|| invalid("scalar match source arm metadata limit"))?;
        let mut arm_spans = Vec::new();
        arm_spans
            .try_reserve_exact(arms.len())
            .map_err(|_| invalid("scalar match source arm allocation"))?;
        for (arm, _) in &arms {
            arm_spans.push(map_syntax(arm.syntax(), context, &origin)?);
        }
        sequence.push(Work::Control(Draft {
            span: origin.clone(),
            exit: DraftExit::Exhausted(ExhaustionSite {
                operand: operand_span,
                scrutinee: scrutinee_span,
                source,
                arms: arm_spans,
            }),
        }));
        sequence.push(Work::Mark(result));
        self.schedule(sequence, stack)
    }
    pub(super) fn if_expression(
        &mut self,
        expression: ast::IfExpr,
        context: Context,
        allocate: &mut impl FnMut() -> Result<SlotId, Diagnostic>,
        stack: &mut Vec<Work>,
        span: &impl Fn(&ast::Expr) -> Result<BodySpan, Diagnostic>,
    ) -> Result<(), Diagnostic> {
        let origin = span(&ast::Expr::IfExpr(expression.clone()))?;
        let yes = self.label()?;
        let no = self.label()?;
        let end = self.label()?;
        let condition = expression.condition().ok_or_else(unsupported)?;
        let mut sequence = self.sequence(if matches!(condition, ast::Expr::LetExpr(_)) {
            40
        } else {
            16
        })?;
        let source = allocate()?;
        if let ast::Expr::LetExpr(condition) = condition {
            sequence.push(Work::Expression(
                condition.expr().ok_or_else(unsupported)?,
                source,
                context.base,
                context.parent,
            ));
            self.pattern(
                Pattern {
                    pattern: condition.pat().ok_or_else(unsupported)?,
                    source,
                    yes,
                    no,
                    origin: &origin,
                },
                context,
                allocate,
                &mut sequence,
                span,
            )?;
        } else {
            sequence.push(Work::Operation(Op::SetHint {
                child: source,
                hint: HintRecipe::Primitive(PrimitiveType::Bool),
            }));
            sequence.push(Work::Expression(
                condition,
                source,
                context.base,
                context.parent,
            ));
            sequence.push(Self::branch(&origin, source, yes, no));
        }
        sequence.push(Work::Mark(yes));
        sequence.push(Work::Operation(Op::Discard { source }));
        sequence.push(Work::Expression(
            ast::Expr::BlockExpr(expression.then_branch().ok_or_else(unsupported)?),
            context.destination,
            context.base,
            context.parent,
        ));
        sequence.push(Self::jump(&origin, end));
        sequence.push(Work::Mark(no));
        sequence.push(Work::Operation(Op::Discard { source }));
        match expression.else_branch() {
            Some(ast::ElseBranch::Block(block)) => sequence.push(Work::Expression(
                ast::Expr::BlockExpr(block),
                context.destination,
                context.base,
                context.parent,
            )),
            Some(ast::ElseBranch::IfExpr(nested)) => sequence.push(Work::Expression(
                ast::Expr::IfExpr(nested),
                context.destination,
                context.base,
                context.parent,
            )),
            None => sequence.push(Work::Operation(Op::Unit {
                destination: context.destination,
            })),
        }
        sequence.push(Work::Mark(end));
        self.schedule(sequence, stack)
    }
    fn sequence(&self, count: usize) -> Result<Vec<Work>, Diagnostic> {
        self.limit
            .checked_mul(
                std::mem::size_of::<Work>()
                    .checked_add(std::mem::size_of::<RuntimeScalarStep>())
                    .ok_or_else(|| invalid("scalar control phase bytes overflow"))?,
            )
            .and_then(|n| n.checked_add(count.checked_mul(std::mem::size_of::<Work>())?))
            .filter(|n| *n <= 32 << 20 && count <= self.limit)
            .ok_or_else(|| invalid("scalar control source work budget"))?;
        let mut work = Vec::new();
        work.try_reserve_exact(count)
            .map_err(|_| invalid("scalar control source work allocation"))?;
        Ok(work)
    }
    fn schedule(&self, sequence: Vec<Work>, stack: &mut Vec<Work>) -> Result<(), Diagnostic> {
        sequence
            .len()
            .checked_add(stack.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| invalid("scalar control work reservation exhausted"))?;
        stack
            .try_reserve(sequence.len())
            .map_err(|_| invalid("scalar control work allocation"))?;
        stack.extend(sequence.into_iter().rev());
        Ok(())
    }
    fn pattern(
        &mut self,
        pattern: Pattern<'_>,
        context: Context,
        allocate: &mut impl FnMut() -> Result<SlotId, Diagnostic>,
        sequence: &mut Vec<Work>,
        span: &impl Fn(&ast::Expr) -> Result<BodySpan, Diagnostic>,
    ) -> Result<(), Diagnostic> {
        if pattern
            .pattern
            .syntax()
            .children()
            .any(|child| ast::Attr::can_cast(child.kind()))
        {
            sequence.push(Work::Fault(
                unsupported(),
                map_syntax(pattern.pattern.syntax(), context, pattern.origin)?,
            ));
            return Ok(());
        }
        let endpoints = match &pattern.pattern {
            ast::Pat::WildcardPat(_) => {
                sequence.push(Self::jump(pattern.origin, pattern.yes));
                return Ok(());
            }
            ast::Pat::LiteralPat(p) => vec![(ast::Pat::LiteralPat(p.clone()), NumericBinary::Eq)],
            ast::Pat::RangePat(p) => {
                let operator = match p.op_kind() {
                    Some(RangeOp::Inclusive) => NumericBinary::Le,
                    Some(RangeOp::Exclusive) => NumericBinary::Lt,
                    None => return Err(invalid("scalar range operator missing")),
                };
                let lower = p.start();
                let upper = p.end();
                if lower.is_none() && upper.is_none()
                    || operator == NumericBinary::Le && upper.is_none()
                {
                    return Err(invalid("scalar range required endpoint missing"));
                }
                let mut endpoints = Vec::new();
                if let Some(lower) = lower {
                    endpoints.push((lower, NumericBinary::Ge));
                }
                if let Some(upper) = upper {
                    endpoints.push((upper, operator));
                }
                endpoints
            }
            _ => {
                sequence.push(Work::Fault(
                    unsupported(),
                    map_syntax(pattern.pattern.syntax(), context, pattern.origin)?,
                ));
                return Ok(());
            }
        };
        let expected = allocate()?;
        let test = allocate()?;
        for (ordinal, (endpoint, operator)) in endpoints.iter().enumerate() {
            let success = if ordinal + 1 == endpoints.len() {
                pattern.yes
            } else {
                self.label()?
            };
            // Semantic unsupported endpoints are reached steps, including a
            // future upper endpoint after a false lower comparison.
            let literal = match endpoint {
                ast::Pat::LiteralPat(endpoint) => pattern_literal(endpoint, context, span),
                _ => Err(unsupported()),
            };
            let literal = match literal {
                Ok(literal) => literal,
                Err(error) if error.code == "LRR-E-UNSUPPORTED" => {
                    sequence.push(Work::Fault(
                        error,
                        map_syntax(endpoint.syntax(), context, pattern.origin)?,
                    ));
                    if ordinal + 1 != endpoints.len() {
                        sequence.push(Work::Mark(success));
                    }
                    continue;
                }
                Err(error) => return Err(error),
            };
            let pass = self.label()?;
            let fail = self.label()?;
            sequence.push(Work::Operation(Op::NumericHint {
                parent: context.destination,
                child: expected,
                left: Some(pattern.source),
                operator: Some(*operator),
            }));
            sequence.push(Work::Operation(Op::Literal {
                literal,
                hint: HintRecipe::None,
                destination: expected,
            }));
            sequence.push(Work::Operation(Op::NumericBinary {
                operator: *operator,
                left: pattern.source,
                right: expected,
                destination: test,
            }));
            sequence.push(Work::Operation(Op::Discard { source: expected }));
            sequence.push(Self::branch(pattern.origin, test, pass, fail));
            sequence.push(Work::Mark(pass));
            sequence.push(Work::Operation(Op::Discard { source: test }));
            sequence.push(Self::jump(pattern.origin, success));
            sequence.push(Work::Mark(fail));
            sequence.push(Work::Operation(Op::Discard { source: test }));
            sequence.push(Self::jump(pattern.origin, pattern.no));
            if ordinal + 1 != endpoints.len() {
                sequence.push(Work::Mark(success));
            }
        }
        Ok(())
    }
}
struct Pattern<'a> {
    pattern: ast::Pat,
    source: SlotId,
    yes: Label,
    no: Label,
    origin: &'a BodySpan,
}
fn map_syntax(
    syntax: &ra_ap_syntax::SyntaxNode,
    context: Context,
    origin: &BodySpan,
) -> Result<BodySpan, Diagnostic> {
    let r = syntax.text_range();
    let offset = |at: TextSize| {
        u32::from(at)
            .checked_sub(u32::try_from("fn __lrr_operand(){let __lrr_value=".len()).ok()?)
            .and_then(|n| context.base.checked_add(n))
    };
    Ok(BodySpan {
        body_key: origin.body_key.clone(),
        relative_start: offset(r.start())
            .ok_or_else(|| invalid("scalar fault origin start overflow"))?,
        relative_end: offset(r.end()).ok_or_else(|| invalid("scalar fault origin end overflow"))?,
    })
}
fn pattern_literal(
    pattern: &ast::LiteralPat,
    context: Context,
    span: &impl Fn(&ast::Expr) -> Result<BodySpan, Diagnostic>,
) -> Result<LiteralRecipe, Diagnostic> {
    let literal = pattern.literal().ok_or_else(unsupported)?;
    let mut origin = span(&ast::Expr::Literal(literal.clone()))?;
    if pattern.minus_token().is_none() {
        return literal_recipe(&literal, origin);
    }
    let minus = pattern.minus_token().ok_or_else(unsupported)?;
    origin.relative_start = u32::from(minus.text_range().start())
        .checked_sub(
            u32::try_from("fn __lrr_operand(){let __lrr_value=".len())
                .map_err(|_| invalid("scalar pattern prefix overflow"))?,
        )
        .and_then(|offset| context.base.checked_add(offset))
        .ok_or_else(|| invalid("scalar pattern origin overflow"))?;
    let integer = ast::IntNumber::cast(literal.token()).ok_or_else(unsupported)?;
    Ok(LiteralRecipe {
        kind: LiteralKind::NegativeInteger,
        spelling: pattern.syntax().text().to_string(),
        source: origin,
        decoded: Some(super::super::literal_decode::integer(&integer, true)),
    })
}

/// Compare the decoder subtree with its exact original operand bytes before
/// retaining any branch proof. No synthetic wrapper identity becomes a source.
pub(super) fn require_written(
    parsed: &ParsedFile,
    body: &BodyHandle,
    span: &BodySpan,
    syntax: &ra_ap_syntax::SyntaxNode,
) -> Result<(), Diagnostic> {
    if span.body_key != body.key() || span.relative_start >= span.relative_end {
        return Err(invalid("scalar control original span differs"));
    }
    let start = body
        .range
        .start
        .checked_add(span.relative_start)
        .ok_or_else(|| invalid("scalar control source start overflow"))?;
    let end = body
        .range
        .start
        .checked_add(span.relative_end)
        .ok_or_else(|| invalid("scalar control source end overflow"))?;
    if end > body.range.end || end - start > 65_500 {
        return Err(invalid("scalar control source range limit"));
    }
    let original = parsed.parsed.tree().syntax().text();
    if original
        .slice(TextRange::new(start.into(), end.into()))
        .to_string()
        != syntax.text().to_string()
    {
        return Err(invalid("scalar control original written source differs"));
    }
    Ok(())
}
#[cfg(test)]
mod authority_tests {
    use super::*;
    #[test]
    fn genuine_control_refuses_modified_condition_target_and_span_without_remint() {
        let (parsed, body, index, sites) = super::super::control_tests::entered(
            "fn main(){assert_eq!(true,match 1_i32{1_i32=>true,_=>false});}",
            2024,
        );
        let (site, args) = &sites[0];
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, site, args)
            .unwrap();
        let original = plan
            .steps()
            .iter()
            .find_map(|step| match step {
                RuntimeScalarStep::Control(control)
                    if matches!(control.exit, SelectedExit::Branch { .. }) =>
                {
                    Some(control.as_ref())
                }
                _ => None,
            })
            .unwrap();
        let copy = || RuntimeScalarControl {
            source: original.source.clone(),
            body_index: original.body_index,
            invocation: original.invocation,
            body_start: original.body_start,
            span: original.span.clone(),
            key: original.key.clone(),
            exit: match &original.exit {
                SelectedExit::Exhausted(site) => SelectedExit::Exhausted(site.clone()),
                SelectedExit::Next(next) => SelectedExit::Next(next.clone()),
                SelectedExit::Branch { condition, yes, no } => SelectedExit::Branch {
                    condition: *condition,
                    yes: yes.clone(),
                    no: no.clone(),
                },
            },
            identity: original.identity,
        };
        original
            .require(&parsed.key(), &index, &original.key)
            .unwrap();
        let mut wrong = copy();
        let SelectedExit::Branch { yes, no, .. } = &mut wrong.exit else {
            panic!()
        };
        std::mem::swap(yes, no);
        assert!(wrong.require(&parsed.key(), &index, &wrong.key).is_err());
        let mut wrong = copy();
        let SelectedExit::Branch { condition, .. } = &mut wrong.exit else {
            panic!()
        };
        *condition = index.nodes()[site.node as usize]
            .runtime_scalar_slots
            .iter()
            .copied()
            .find(|slot| *slot != *condition)
            .unwrap();
        assert!(wrong.require(&parsed.key(), &index, &wrong.key).is_err());
        let mut wrong = copy();
        wrong.span.relative_end -= 1;
        assert!(wrong.require(&parsed.key(), &index, &wrong.key).is_err());
        let mut wrong = copy();
        wrong.key.phase = Phase::Commit.code().unwrap();
        assert!(wrong.require(&parsed.key(), &index, &wrong.key).is_err());
    }
}

#[cfg(test)]
mod exhaustion_authority_tests {
    use super::*;
    #[test]
    fn match_exhaustion_cannot_be_reminted_by_copying_a_phase_slot_or_source_range() {
        let (parsed, body, index, sites) = super::super::control_tests::entered(
            "fn main(){assert_eq!(true,match false{true=>true});}",
            2024,
        );
        let (key, args) = &sites[0];
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, key, args)
            .unwrap();
        let original = plan
            .steps()
            .iter()
            .find_map(|step| match step {
                RuntimeScalarStep::Control(control) if control.exhaustion().is_some() => {
                    Some(control.as_ref())
                }
                _ => None,
            })
            .unwrap();
        let copy = || RuntimeScalarControl {
            source: original.source.clone(),
            body_index: original.body_index,
            invocation: original.invocation,
            body_start: original.body_start,
            span: original.span.clone(),
            key: original.key.clone(),
            identity: original.identity,
            exit: match &original.exit {
                SelectedExit::Exhausted(site) => SelectedExit::Exhausted(site.clone()),
                _ => panic!(),
            },
        };
        original
            .exhaustion()
            .unwrap()
            .require(&index, &original.key)
            .unwrap();
        let mut wrong = copy();
        let SelectedExit::Exhausted(site) = &mut wrong.exit else {
            panic!()
        };
        site.source = SlotId(site.source.0 + 2);
        assert!(
            wrong
                .exhaustion()
                .unwrap()
                .require(&index, &wrong.key)
                .is_err()
        );
        let mut wrong = copy();
        let SelectedExit::Exhausted(site) = &mut wrong.exit else {
            panic!()
        };
        site.arms.clear();
        assert!(
            wrong
                .exhaustion()
                .unwrap()
                .require(&index, &wrong.key)
                .is_err()
        );
        let mut wrong = copy();
        wrong.span.relative_start += 1;
        assert!(
            wrong
                .exhaustion()
                .unwrap()
                .require(&index, &wrong.key)
                .is_err()
        );
        let mut wrong = copy();
        wrong.key.phase = Phase::Entry.code().unwrap();
        assert!(
            wrong
                .exhaustion()
                .unwrap()
                .require(&index, &wrong.key)
                .is_err()
        );
        let mut wrong = copy();
        wrong.source.content = Digest([163; 32]);
        assert!(
            wrong
                .exhaustion()
                .unwrap()
                .require(&index, &wrong.key)
                .is_err()
        );
        assert!(
            plan.steps()
                .iter()
                .filter_map(|step| match step {
                    RuntimeScalarStep::Control(control)
                        if !matches!(control.exit, SelectedExit::Exhausted(_)) =>
                        Some(control),
                    _ => None,
                })
                .all(|control| control.exhaustion().is_none())
        );
    }
}
