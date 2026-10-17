//! Separate pure constant evaluator. Never allocates live guest state or invokes a provider.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::op::{DecodedLiteral, NumericBinary, PrimitiveType},
    runtime::{
        cell::GuestStore,
        shape::{PrimitiveIdentity, TypeId},
        value::ValueRepr,
    },
    syntax::{
        pure_constants::{
            ConstType, Constant, ConstantPlan, ConstantSource, ControlTarget, ExprRef, Function,
            Node, Statement, StatementRef,
        },
        spans::SourceRange,
    },
    vm::numeric::{self, NumericFailure},
};
use std::collections::{BTreeMap, BTreeSet};
// E-R5-006: the only constant-evaluation limit that rustc itself enforces is the
// stack-frame limit, which is the crate `recursion_limit` (default 128;
// compiler/rustc_const_eval/src/const_eval/machine.rs, `before_stack_push`).
// Evaluation steps and result bytes are not limited.
const MAX_STEPS: u32 = u32::MAX;
const MAX_FRAMES: u16 = 128;
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub use crate::ir::constant_data::ConstValue;
#[path = "constant_locals.rs"]
mod locals;
use locals::LocalScopes;

#[derive(Debug, Clone, Copy)]
pub struct ConstLimits {
    steps: u32,
    frames: u16,
    result_bytes: usize,
}
impl Default for ConstLimits {
    fn default() -> Self {
        Self {
            steps: MAX_STEPS,
            frames: MAX_FRAMES,
            result_bytes: usize::MAX,
        }
    }
}
impl ConstLimits {
    /// Retried evaluation uses the same total host-step budget.
    pub(crate) fn after_steps(self, spent: u32) -> Result<Self, Diagnostic> {
        let steps = self
            .steps
            .checked_sub(spent)
            .filter(|n| *n > 0)
            .ok_or_else(|| fault("LRR-E-CONST-LIMIT", "constant retry step budget exhausted"))?;
        Ok(Self { steps, ..self })
    }
    pub fn new(steps: u32, frames: u16, result_bytes: usize) -> Result<Self, Diagnostic> {
        if steps == 0 || frames == 0 || frames > MAX_FRAMES || result_bytes == 0 {
            return Err(fault(
                "LRR-E-CONST-LIMIT",
                "constant limits exceed the live profile",
            ));
        }
        Ok(Self {
            steps,
            frames,
            result_bytes,
        })
    }
}
#[derive(Debug, Clone)]
pub struct ConstDependency {
    definition: Digest,
    body: crate::source::manifest::BodyKey,
    arguments: Vec<Digest>,
}
impl ConstDependency {
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn body(&self) -> &crate::source::manifest::BodyKey {
        &self.body
    }
    pub fn arguments(&self) -> &[Digest] {
        &self.arguments
    }
}
impl StableEncode for ConstDependency {
    const DOMAIN: &'static str = "lrr/v1/pure-constant-dependency";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.record(&self.body);
        e.sequence(&self.arguments, |e, a| e.digest(*a))
    }
}
#[derive(Debug)]
pub struct ConstEvaluation {
    value: ConstValue,
    value_type: wire::ConstantWireType,
    source: Digest,
    cfg: Digest,
    plan: Digest,
    dependencies: Vec<ConstDependency>,
    steps: u32,
}
impl ConstEvaluation {
    pub fn value(&self) -> &ConstValue {
        &self.value
    }
    /// Declared concrete result shape, retained even when the value has no elements.
    pub fn value_type(&self) -> &wire::ConstantWireType {
        &self.value_type
    }
    pub fn source(&self) -> Digest {
        self.source
    }
    pub fn cfg(&self) -> Digest {
        self.cfg
    }
    pub fn plan(&self) -> Digest {
        self.plan
    }
    pub fn dependencies(&self) -> &[ConstDependency] {
        &self.dependencies
    }
    pub fn steps(&self) -> u32 {
        self.steps
    }
    /// Pure admission calculation of the exact array length.
    pub fn admit_array_length(&self) -> Result<usize, Diagnostic> {
        let value = self
            .value
            .scalar()
            .ok_or_else(|| fault("LRR-E-CONST-TYPE", "array length is not scalar"))?;
        if value.kind != 5 || value.type_id != 13 {
            return Err(fault(
                "LRR-E-CONST-TYPE",
                "array length is not exact nonnegative usize",
            ));
        }
        let count = usize::try_from(value.payload)
            .map_err(|_| fault("LRR-E-CONST-LIMIT", "array length exceeds host width"))?;
        Ok(count)
    }
}
#[derive(Debug, Clone)]
pub struct ConstPanic {
    message: String,
    definition: Digest,
    proof: Digest,
}
impl ConstPanic {
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn proof(&self) -> Digest {
        self.proof
    }
}
#[derive(Debug)]
pub struct ConstFailure {
    diagnostic: Box<Diagnostic>,
    source: Digest,
    range: SourceRange,
    panic: Option<Box<ConstPanic>>,
    numeric_panic: Option<crate::vm::numeric::PanicKind>,
    requested_plan: Option<Digest>,
    required_macro: Option<Box<crate::syntax::pure_constants::RequiredConstantMacro>>,
    evaluation_steps: u32,
}
impl ConstFailure {
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    pub fn source(&self) -> Digest {
        self.source
    }
    pub fn range(&self) -> SourceRange {
        self.range
    }
    pub fn requested_plan(&self) -> Option<Digest> {
        self.requested_plan
    }
    pub(crate) fn required_macro(
        &self,
    ) -> Option<&crate::syntax::pure_constants::RequiredConstantMacro> {
        self.required_macro.as_deref()
    }
    pub(crate) fn evaluation_steps(&self) -> u32 {
        self.evaluation_steps
    }
    pub fn numeric_panic(&self) -> Option<crate::vm::numeric::PanicKind> {
        self.numeric_panic
    }
    pub fn panic(&self) -> Option<&ConstPanic> {
        self.panic.as_deref()
    }
}
#[derive(Debug)]
enum Flow {
    Value(ConstValue),
    Return(ConstValue),
    Break(ControlTarget, ConstValue),
    Continue(ControlTarget),
}
impl Flow {
    fn into_value(self) -> Result<ConstValue, Diagnostic> {
        match self {
            Self::Value(v) | Self::Return(v) => Ok(v),
            Self::Break(..) | Self::Continue(..) => Err(fault(
                "LRR-E-CONST-SOURCE",
                "constant loop exit escaped its source activation",
            )),
        }
    }
}
#[derive(Clone, Copy)]
enum Expected {
    Scalar(PrimitiveType),
    Array(PrimitiveType, usize),
}
struct Evaluator<'a> {
    source: &'a ConstantSource,
    limits: ConstLimits,
    steps: u32,
    frames: u16,
    depth: u16,
    returns: Vec<Expected>,
    loops: Vec<(ControlTarget, Option<Expected>)>,
    metadata_bytes: usize,
    active: BTreeSet<Digest>,
    active_calls: BTreeSet<Digest>,
    dependencies: BTreeMap<Digest, ConstDependency>,
    nodes: BTreeMap<(u32, u32), Node>,
    local_contexts: BTreeMap<(u32, u32), PrimitiveType>,
    local_context_work: usize,
}
macro_rules! take {
    ($this:ident,$expr:expr,$hint:expr,$locals:expr,$allow:expr) => {
        match $this.expr($expr, $hint, $locals, $allow)? {
            Flow::Value(v) => v,
            other => return Ok(other),
        }
    };
}
fn primitive_id(ty: PrimitiveType) -> Result<TypeId, Diagnostic> {
    Ok(PrimitiveIdentity::from_recipe(ty)?.id())
}
fn scalar(v: ConstValue) -> Result<ValueRepr, Diagnostic> {
    v.scalar()
        .ok_or_else(|| fault("LRR-E-CONST-TYPE", "constant operation requires a scalar"))
}
impl Evaluator<'_> {
    fn failure(&self, diagnostic: Diagnostic, range: SourceRange) -> ConstFailure {
        ConstFailure {
            diagnostic: Box::new(diagnostic),
            source: self.source.source(),
            range,
            panic: None,
            numeric_panic: None,
            requested_plan: None,
            required_macro: None,
            evaluation_steps: self.steps,
        }
    }
    fn limit(&self, at: SourceRange) -> ConstFailure {
        self.failure(
            fault("LRR-E-CONST-LIMIT", "constant resource limit reached"),
            at,
        )
    }
    fn checked_value(
        &self,
        value: ConstValue,
        _at: SourceRange,
    ) -> Result<ConstValue, ConstFailure> {
        Ok(value)
    }
    fn node(&mut self, expr: &ExprRef) -> Result<Node, ConstFailure> {
        if let Some(n) = self.nodes.get(&expr.key()) {
            let bytes = n.owned_bytes().ok_or_else(|| self.limit(expr.range()))?;
            self.metadata_bytes = self
                .metadata_bytes
                .checked_add(bytes)
                .ok_or_else(|| self.limit(expr.range()))?;
            return Ok(n.clone());
        }
        let n = self
            .source
            .node(expr)
            .map_err(|d| self.failure(d, expr.range()))?;
        let bytes = n.owned_bytes().ok_or_else(|| self.limit(expr.range()))?;
        self.admit_metadata(bytes, expr.range())?;
        self.nodes.insert(expr.key(), n.clone());
        Ok(n)
    }
    fn numeric(
        &self,
        result: Result<ValueRepr, NumericFailure>,
        at: SourceRange,
    ) -> Result<ConstValue, ConstFailure> {
        let v = result.map_err(|error| match error {
            NumericFailure::GuestPanic(kind) => {
                let mut failure = self.failure(fault("LRR-E-CONST-PANIC", kind.description()), at);
                failure.numeric_panic = Some(kind);
                failure
            }
            NumericFailure::RuntimeFault(diagnostic) => self.failure(diagnostic, at),
        })?;
        ConstValue::from_scalar(v).map_err(|d| self.failure(d, at))
    }
    fn ty(&mut self, ty: &ConstType, at: SourceRange) -> Result<Expected, ConstFailure> {
        match ty {
            ConstType::Scalar(p) => Ok(Expected::Scalar(*p)),
            ConstType::Array(p, len) => {
                let value = self
                    .expr(
                        len,
                        Some(Expected::Scalar(PrimitiveType::Usize)),
                        &mut LocalScopes::default(),
                        false,
                    )?
                    .into_value()
                    .map_err(|d| self.failure(d, at))?;
                let count = self.length(value, at)?;
                Ok(Expected::Array(*p, count))
            }
        }
    }
    fn length(&self, value: ConstValue, at: SourceRange) -> Result<usize, ConstFailure> {
        let scalar = scalar(value).map_err(|d| self.failure(d, at))?;
        if scalar.kind != 5 || scalar.type_id != 13 {
            return Err(self.failure(
                fault(
                    "LRR-E-CONST-TYPE",
                    "array length requires exact nonnegative usize",
                ),
                at,
            ));
        }
        usize::try_from(scalar.payload).map_err(|_| self.limit(at))
    }
    fn admit_metadata(&mut self, bytes: usize, at: SourceRange) -> Result<(), ConstFailure> {
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(bytes)
            .ok_or_else(|| self.limit(at))?;
        Ok(())
    }
    fn dependency(
        &mut self,
        declaration: &crate::syntax::pure_constants::ConstantDeclaration,
        args: &[ConstValue],
        at: SourceRange,
    ) -> Result<(), ConstFailure> {
        let dependency = ConstDependency {
            definition: declaration.definition(),
            body: declaration.body().clone(),
            arguments: args.iter().map(StableEncode::fingerprint).collect(),
        };
        let key = dependency.fingerprint();
        if !self.dependencies.contains_key(&key) {
            self.admit_metadata(
                args.len()
                    .checked_mul(32)
                    .and_then(|n| n.checked_add(160))
                    .ok_or_else(|| self.limit(at))?,
                at,
            )?;
            self.dependencies.insert(key, dependency);
        }
        Ok(())
    }
    fn constant(&mut self, c: Constant) -> Result<(ConstValue, Expected), ConstFailure> {
        let at = c.body.range();
        if !self.active.insert(c.declaration.definition()) {
            return Err(self.failure(
                fault("LRR-E-CONST-CYCLE", "active named constant cycle"),
                at,
            ));
        }
        if self.frames >= self.limits.frames {
            self.active.remove(&c.declaration.definition());
            return Err(self.limit(at));
        }
        self.frames += 1;
        self.dependency(&c.declaration, &[], at)?;
        let previous_depth = std::mem::replace(&mut self.depth, 0);
        let previous_loops = std::mem::take(&mut self.loops);
        let value = (|| {
            let expected = self.ty(&c.ty, at)?;
            let value = self
                .expr(&c.body, Some(expected), &mut LocalScopes::default(), false)?
                .into_value()
                .map_err(|d| self.failure(d, at))?;
            self.check_expected(&value, expected, at)?;
            Ok((value, expected))
        })();
        self.depth = previous_depth;
        self.loops = previous_loops;
        self.frames -= 1;
        self.active.remove(&c.declaration.definition());
        value
    }
    fn function(
        &mut self,
        f: Function,
        args: Vec<ConstValue>,
        at: SourceRange,
    ) -> Result<(ConstValue, Expected), ConstFailure> {
        if self.frames >= self.limits.frames {
            return Err(self.limit(at));
        }
        if f.params.len() != args.len() {
            return Err(self.failure(
                fault(
                    "LRR-E-CONST-TYPE",
                    "const-call argument cardinality differs",
                ),
                at,
            ));
        }
        let bytes = f
            .params
            .iter()
            .try_fold(0usize, |n, (name, _, _)| {
                n.checked_add(name.len())?.checked_add(192)
            })
            .ok_or_else(|| self.limit(at))?;
        self.admit_metadata(
            bytes
                .checked_add(std::mem::size_of::<LocalScopes>() + 64)
                .ok_or_else(|| self.limit(at))?,
            at,
        )?;
        let mut locals = LocalScopes::default();
        locals.push().map_err(|d| self.failure(d, at))?;
        for ((name, ty, mutable), value) in f.params.iter().zip(&args) {
            if !value.matches_scalar(*ty).map_err(|d| self.failure(d, at))? {
                return Err(self.failure(
                    fault("LRR-E-CONST-TYPE", "const-call argument type differs"),
                    at,
                ));
            }
            locals
                .insert(name.clone(), *mutable, value.clone())
                .map_err(|d| self.failure(d, at))?;
        }
        let mut encoder = StableEncoder::new("lrr/v1/active-pure-const-call");
        encoder.digest(f.declaration.definition());
        encoder.record(f.declaration.body());
        encoder.sequence(&args, |e, v| e.record(v));
        let call_key = encoder.finish();
        if !self.active_calls.insert(call_key) {
            return Err(self.failure(
                fault(
                    "LRR-E-CONST-CYCLE",
                    "active const-call cycle with identical pure arguments",
                ),
                at,
            ));
        }
        self.frames += 1;
        self.dependency(&f.declaration, &args, at)?;
        let previous_depth = std::mem::replace(&mut self.depth, 0);
        let previous_loops = std::mem::take(&mut self.loops);
        let value = (|| {
            let expected = self.ty(&f.result, at)?;
            self.returns.push(expected);
            let value = self.expr(&f.body, Some(expected), &mut locals, true);
            self.returns.pop();
            let value = value?.into_value().map_err(|d| self.failure(d, at))?;
            self.check_expected(&value, expected, at)?;
            Ok((value, expected))
        })();
        self.depth = previous_depth;
        self.loops = previous_loops;
        self.frames -= 1;
        self.active_calls.remove(&call_key);
        value
    }
    fn hint(
        &mut self,
        e: &ExprRef,
        locals: &LocalScopes,
        depth: u16,
    ) -> Result<Option<PrimitiveType>, ConstFailure> {
        Ok(match self.node(e)? {
            Node::Bool(_) | Node::Logic(..) => Some(PrimitiveType::Bool),
            Node::Char(_) => Some(PrimitiveType::Char),
            Node::Byte(_) => Some(PrimitiveType::U8),
            Node::Unit => Some(PrimitiveType::Unit),
            Node::Integer(DecodedLiteral::Integer { suffix, .. }) => suffix,
            Node::Float(_, suffix) => suffix,
            Node::Parentheses(e) | Node::Unary(_, e) => self.hint(&e, locals, depth + 1)?,
            Node::Cast(_, p) => Some(p),
            Node::Binary(op, a, b) => {
                if comparison(op) {
                    Some(PrimitiveType::Bool)
                } else {
                    self.hint(&a, locals, depth + 1)?
                        .or(self.hint(&b, locals, depth + 1)?)
                }
            }
            Node::Path(name) => {
                if let Some(v) = locals.get(&name) {
                    primitive_for_value(v.scalar())
                } else {
                    let c = self
                        .source
                        .named_constant(&name)
                        .map_err(|d| self.failure(d, e.range()))?;
                    match c.ty {
                        ConstType::Scalar(p) => Some(p),
                        _ => None,
                    }
                }
            }
            _ => None,
        })
    }
    /// rustc `FnCtxt::check_lit` under `ExpectCastableToType(destination)`
    /// (compiler/rustc_hir_typeck/src/expr.rs): an unsuffixed integer
    /// literal takes an integral cast target (`u8` for a `char` target) and
    /// an unsuffixed float literal a float target. The expectation reaches
    /// the literal through parentheses and unary `-`/`!` only
    /// (`check_expr_unop` keeps it for `Not`/`Neg`). `None` keeps the
    /// literal's own `i32`/`f64` fallback.
    fn cast_expectation(
        &mut self,
        e: &ExprRef,
        destination: PrimitiveType,
    ) -> Result<Option<PrimitiveType>, ConstFailure> {
        use PrimitiveType as P;
        let mut e = e.clone();
        loop {
            match self.node(&e)? {
                Node::Parentheses(child) | Node::Unary(_, child) => e = child,
                Node::Integer(DecodedLiteral::Integer { suffix: None, .. }) => {
                    return Ok(match destination {
                        P::I8
                        | P::I16
                        | P::I32
                        | P::I64
                        | P::I128
                        | P::Isize
                        | P::U8
                        | P::U16
                        | P::U32
                        | P::U64
                        | P::U128
                        | P::Usize => Some(destination),
                        P::Char => Some(P::U8),
                        _ => None,
                    });
                }
                Node::Float(_, None) => {
                    return Ok(matches!(destination, P::F32 | P::F64).then_some(destination));
                }
                _ => return Ok(None),
            }
        }
    }
    fn expr(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
    ) -> Result<Flow, ConstFailure> {
        self.steps = self
            .steps
            .checked_add(1)
            .ok_or_else(|| self.limit(e.range()))?;
        self.depth += 1;
        let result = self.inner(e, hint, locals, allow_return);
        self.depth -= 1;
        result
    }
    // Keep recursively retained host frames small in unoptimized builds.
    // A selected family retains only its own temporaries at each source level.
    #[inline(never)]
    fn inner(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
    ) -> Result<Flow, ConstFailure> {
        let node = self.node(e)?;
        match node {
            Node::Block(..) => self.block_node(e, hint, locals, allow_return, node),
            Node::Loop(target, condition, body) => {
                self.run_loop(target, condition, body, hint, locals, allow_return)
            }
            Node::Break(..) | Node::Continue(..) | Node::If(..) | Node::Return(..) => {
                self.control_node(e, hint, locals, allow_return, node)
            }
            Node::Assign(..)
            | Node::Binary(..)
            | Node::Cast(..)
            | Node::Logic(..)
            | Node::Parentheses(..)
            | Node::Unary(..) => self.numeric_node(e, hint, locals, allow_return, node),
            Node::Array(..) | Node::Repeat(..) => {
                self.aggregate_node(e, hint, locals, allow_return, node)
            }
            Node::Call(..) => self.call_node(e, hint, locals, allow_return, node),
            node => self.leaf_node(e, hint, locals, allow_return, node),
        }
    }
    #[inline(never)]
    fn block_node(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        node: Node,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let Node::Block(statements, tail, target) = node else {
            return Err(self.failure(
                fault("LRR-E-CONST-SOURCE", "constant block dispatch differs"),
                at,
            ));
        };
        let Some(target) = target else {
            return self.run_block(statements, tail, hint, locals, allow_return, at);
        };
        self.activate_control(target, hint, at)?;
        let result = self.run_block(statements, tail, hint, locals, allow_return, at);
        self.loops.pop();
        match result {
            Ok(Flow::Break(current, value)) if current == target => {
                if let Some(expected) = hint {
                    self.check_expected(&value, expected, at)?;
                }
                Ok(Flow::Value(self.checked_value(value, at)?))
            }
            other => other,
        }
    }
    #[inline(never)]
    fn control_node(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        node: Node,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let value = match node {
            Node::If(condition, yes, no) => {
                let v = take!(
                    self,
                    &condition,
                    Some(Expected::Scalar(PrimitiveType::Bool)),
                    locals,
                    allow_return
                );
                let v = scalar(v).map_err(|d| self.failure(d, at))?;
                if v.type_id != 2 {
                    return Err(self.failure(
                        fault("LRR-E-CONST-TYPE", "constant if requires bool"),
                        condition.range(),
                    ));
                }
                if v.payload != 0 {
                    return self.expr(&yes, hint, locals, allow_return);
                }
                if let Some(no) = no {
                    return self.expr(&no, hint, locals, allow_return);
                }
                ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?
            }
            Node::Break(target, child) => {
                let target_hint = self
                    .loops
                    .iter()
                    .rev()
                    .find(|(current, _)| *current == target)
                    .map(|(_, hint)| *hint)
                    .ok_or_else(|| {
                        self.failure(
                            fault(
                                "LRR-E-CONST-SOURCE",
                                "constant break target has no current control activation",
                            ),
                            at,
                        )
                    })?;
                let value = if let Some(child) = child {
                    take!(self, &child, target_hint, locals, allow_return)
                } else {
                    ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?
                };
                if let Some(expected) = target_hint {
                    self.check_expected(&value, expected, at)?;
                }
                return Ok(Flow::Break(target, value));
            }
            Node::Continue(target) => {
                if !target.is_loop() || !self.loops.iter().any(|(current, _)| *current == target) {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-SOURCE",
                            "constant continue target has no current loop activation",
                        ),
                        at,
                    ));
                }
                return Ok(Flow::Continue(target));
            }
            Node::Return(child) => {
                if !allow_return {
                    return Err(self.failure(
                        fault("LRR-E-CONST-UNSUPPORTED", "return outside a pure const fn"),
                        at,
                    ));
                }
                let return_hint = self.returns.last().copied().ok_or_else(|| {
                    self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "constant return has no current frame result context",
                        ),
                        at,
                    )
                })?;
                let v = if let Some(child) = child {
                    take!(self, &child, Some(return_hint), locals, allow_return)
                } else {
                    ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?
                };
                return Ok(Flow::Return(v));
            }
            _ => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-SOURCE",
                        "constant node family dispatch differs",
                    ),
                    at,
                ));
            }
        };
        let value = self.checked_value(value, at)?;
        if let Some(expected) = hint {
            self.check_expected(&value, expected, at)?;
        }
        Ok(Flow::Value(value))
    }
    #[inline(never)]
    fn numeric_node(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        node: Node,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let scalar_hint = match hint {
            Some(Expected::Scalar(p)) => Some(p),
            _ => None,
        };
        let value = match node {
            Node::Parentheses(child) => return self.expr(&child, hint, locals, allow_return),
            Node::Unary(op, child) => {
                let v = take!(self, &child, hint, locals, allow_return);
                let v = scalar(v).map_err(|d| self.failure(d, at))?;
                self.numeric(numeric::unary(op, v), at)?
            }
            Node::Binary(op, a, b) => {
                let inferred = self
                    .hint(&a, locals, 0)?
                    .or(self.hint(&b, locals, 0)?)
                    .or(if comparison(op) { None } else { scalar_hint })
                    .unwrap_or(PrimitiveType::I32);
                let ah = Some(Expected::Scalar(inferred));
                let av = take!(self, &a, ah, locals, allow_return);
                let bv = take!(self, &b, ah, locals, allow_return);
                self.numeric(
                    numeric::binary(
                        op,
                        scalar(av).map_err(|d| self.failure(d, at))?,
                        scalar(bv).map_err(|d| self.failure(d, at))?,
                    ),
                    at,
                )?
            }
            Node::Assign(name, op, rhs) => {
                let primitive = primitive_for_value(locals.get(&name).and_then(ConstValue::scalar))
                    .ok_or_else(|| {
                        self.failure(
                            fault(
                                "LRR-E-CONST-UNSUPPORTED",
                                "assignment requires a current scalar local",
                            ),
                            at,
                        )
                    })?;
                let rhs_hint = if matches!(op, Some(NumericBinary::Shl | NumericBinary::Shr)) {
                    None
                } else {
                    Some(Expected::Scalar(primitive))
                };
                // Primitive assignment evaluates the RHS before reading the
                // LHS. Nested RHS blocks may update this same source binding.
                let rhs_value = take!(self, &rhs, rhs_hint, locals, allow_return);
                let value = if let Some(op) = op {
                    let lhs = locals.get(&name).cloned().ok_or_else(|| {
                        self.failure(
                            fault("LRR-E-CONST-SOURCE", "assignment binding disappeared"),
                            at,
                        )
                    })?;
                    self.numeric(
                        numeric::binary(
                            op,
                            scalar(lhs).map_err(|d| self.failure(d, at))?,
                            scalar(rhs_value).map_err(|d| self.failure(d, at))?,
                        ),
                        at,
                    )?
                } else {
                    rhs_value
                };
                self.check_expected(&value, Expected::Scalar(primitive), at)?;
                let value = self.checked_value(value, at)?;
                locals
                    .replace(&name, value)
                    .map_err(|d| self.failure(d, at))?;
                ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?
            }
            Node::Logic(and, a, b) => {
                let av = take!(
                    self,
                    &a,
                    Some(Expected::Scalar(PrimitiveType::Bool)),
                    locals,
                    allow_return
                );
                let v = scalar(av).map_err(|d| self.failure(d, at))?;
                if v.type_id != 2 {
                    return Err(
                        self.failure(fault("LRR-E-CONST-TYPE", "lazy boolean requires bool"), at)
                    );
                }
                if (v.payload != 0) == and {
                    take!(
                        self,
                        &b,
                        Some(Expected::Scalar(PrimitiveType::Bool)),
                        locals,
                        allow_return
                    )
                } else {
                    ConstValue::from_scalar(v).map_err(|d| self.failure(d, at))?
                }
            }
            Node::Cast(child, destination) => {
                // An operand without its own proven type takes rustc's cast
                // expectation (`check_lit` under `ExpectCastableToType`);
                // anything else keeps its ordinary literal fallback.
                let proven = match self.hint(&child, locals, 0)? {
                    Some(proven) => Some(proven),
                    None => self.cast_expectation(&child, destination)?,
                };
                let v = take!(
                    self,
                    &child,
                    proven.map(Expected::Scalar),
                    locals,
                    allow_return
                );
                self.numeric(
                    numeric::cast(
                        scalar(v).map_err(|d| self.failure(d, at))?,
                        primitive_id(destination).map_err(|d| self.failure(d, at))?,
                    ),
                    at,
                )?
            }
            _ => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-SOURCE",
                        "constant node family dispatch differs",
                    ),
                    at,
                ));
            }
        };
        let value = self.checked_value(value, at)?;
        if let Some(expected) = hint {
            self.check_expected(&value, expected, at)?;
        }
        Ok(Flow::Value(value))
    }
    #[inline(never)]
    fn aggregate_node(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        node: Node,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let value = match node {
            Node::Array(elements) => {
                let Some(Expected::Array(p, len)) = hint else {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "constant array needs exact declared element/length context",
                        ),
                        at,
                    ));
                };
                if elements.len() != len {
                    return Err(self.failure(
                        fault("LRR-E-CONST-TYPE", "constant array cardinality differs"),
                        at,
                    ));
                }
                self.array_admission(len, 17, at)?;
                let mut values = Vec::new();
                values.try_reserve_exact(len).map_err(|_| self.limit(at))?;
                for e in elements {
                    values.push(take!(
                        self,
                        &e,
                        Some(Expected::Scalar(p)),
                        locals,
                        allow_return
                    ));
                }
                ConstValue::from_elements(values).map_err(|_| self.limit(at))?
            }
            Node::Repeat(initializer, len) => {
                let Some(Expected::Array(p, expected)) = hint else {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "constant repeat needs exact declared element/length context",
                        ),
                        at,
                    ));
                };
                let count = take!(
                    self,
                    &len,
                    Some(Expected::Scalar(PrimitiveType::Usize)),
                    locals,
                    allow_return
                );
                let count = self.length(count, len.range())?;
                if count != expected {
                    return Err(self.failure(
                        fault("LRR-E-CONST-TYPE", "constant repeat length differs"),
                        at,
                    ));
                }
                self.array_admission(count, 17, at)?;
                let value = take!(
                    self,
                    &initializer,
                    Some(Expected::Scalar(p)),
                    locals,
                    allow_return
                );
                let mut values = Vec::new();
                values
                    .try_reserve_exact(count)
                    .map_err(|_| self.limit(at))?;
                values.resize(count, value);
                ConstValue::from_elements(values).map_err(|_| self.limit(at))?
            }
            _ => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-SOURCE",
                        "constant node family dispatch differs",
                    ),
                    at,
                ));
            }
        };
        let value = self.checked_value(value, at)?;
        if let Some(expected) = hint {
            self.check_expected(&value, expected, at)?;
        }
        Ok(Flow::Value(value))
    }
    #[inline(never)]
    fn call_node(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        node: Node,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let value = match node {
            Node::Call(name, args) => {
                if locals.contains_key(&name) {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "local scalar shadow is not a callable constant definition",
                        ),
                        at,
                    ));
                }
                let f = self
                    .source
                    .named_function(&name)
                    .map_err(|d| self.failure(d, at))?;
                if args.len() != f.params.len() {
                    return Err(
                        self.failure(fault("LRR-E-CONST-TYPE", "const-call arity differs"), at)
                    );
                }
                let mut values = Vec::new();
                for (arg, (_, ty, _)) in args.iter().zip(&f.params) {
                    values.push(take!(
                        self,
                        arg,
                        Some(Expected::Scalar(*ty)),
                        locals,
                        allow_return
                    ));
                }
                self.function(f, values, at)?.0
            }
            _ => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-SOURCE",
                        "constant node family dispatch differs",
                    ),
                    at,
                ));
            }
        };
        let value = self.checked_value(value, at)?;
        if let Some(expected) = hint {
            self.check_expected(&value, expected, at)?;
        }
        Ok(Flow::Value(value))
    }
    #[inline(never)]
    fn leaf_node(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        _allow_return: bool,
        node: Node,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let scalar_hint = match hint {
            Some(Expected::Scalar(p)) => Some(p),
            _ => None,
        };
        let value = match node {
            Node::Integer(DecodedLiteral::Integer {
                negative,
                magnitude,
                suffix,
                ..
            }) => {
                let ty = suffix.or(scalar_hint).unwrap_or(PrimitiveType::I32);
                if suffix.is_some() && scalar_hint.is_some_and(|p| Some(p) != suffix) {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "constant integer suffix conflicts with context",
                        ),
                        at,
                    ));
                }
                // A reached source context must belong to the literal's
                // family before it reaches descriptor/payload construction.
                // Keep 128-bit recipes on their existing unsupported-width
                // admission path, rather than reclassifying integer validity.
                if !matches!(
                    ty,
                    PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::I128
                        | PrimitiveType::Isize
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                        | PrimitiveType::U128
                        | PrimitiveType::Usize
                ) {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "constant integer requires an integer source context",
                        ),
                        at,
                    ));
                }
                let id = primitive_id(ty).map_err(|d| self.failure(d, at))?;
                ConstValue::from_scalar(
                    GuestStore::preview_integer(negative, magnitude, id)
                        .map_err(|d| self.failure(d, at))?,
                )
                .map_err(|d| self.failure(d, at))?
            }
            Node::Integer(_) => {
                return Err(self.failure(
                    fault("LRR-E-CONST-TYPE", "invalid constant integer spelling"),
                    at,
                ));
            }
            Node::Bool(b) => {
                ConstValue::from_scalar(ValueRepr::boolean(b)).map_err(|d| self.failure(d, at))?
            }
            Node::Char(c) => {
                ConstValue::from_scalar(ValueRepr::character(c)).map_err(|d| self.failure(d, at))?
            }
            Node::Byte(b) => ConstValue::from_scalar(
                ValueRepr::unsigned(TypeId(9), u64::from(b)).map_err(|d| self.failure(d, at))?,
            )
            .map_err(|d| self.failure(d, at))?,
            Node::Unit => {
                ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?
            }
            Node::Float(text, suffix) => {
                let p = suffix.or(scalar_hint).unwrap_or(PrimitiveType::F64);
                if scalar_hint
                    .is_some_and(|hint| !matches!(hint, PrimitiveType::F32 | PrimitiveType::F64))
                    || !matches!(p, PrimitiveType::F32 | PrimitiveType::F64)
                {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "constant float requires a floating source context",
                        ),
                        at,
                    ));
                }
                ConstValue::from_scalar(
                    GuestStore::preview_float(
                        &text,
                        primitive_id(p).map_err(|d| self.failure(d, at))?,
                    )
                    .map_err(|d| self.failure(d, at))?,
                )
                .map_err(|d| self.failure(d, at))?
            }
            Node::Path(name) => {
                if let Some(v) = locals.get(&name) {
                    v.clone()
                } else {
                    let c = self
                        .source
                        .named_constant(&name)
                        .map_err(|d| self.failure(d, at))?;
                    self.constant(c)?.0
                }
            }
            Node::RequiredMacro(site) => {
                let mut failure = self.failure(
                    fault(
                        "LRR-E-CONST-MACRO-SOURCE",
                        "reached constant macro requires current source/operand admission",
                    ),
                    at,
                );
                failure.required_macro = Some(site);
                return Err(failure);
            }
            Node::AdmittedPanic(plan) => {
                if plan.range() != at {
                    return Err(self.failure(
                        fault(
                            "LRR-E-CONST-MACRO-SOURCE",
                            "constant panic proof range differs",
                        ),
                        at,
                    ));
                }
                let bytes = plan
                    .message()
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(std::mem::size_of::<ConstPanic>() + 128))
                    .ok_or_else(|| self.limit(at))?;
                self.admit_metadata(bytes, at)?;
                let mut failure = self.failure(fault("LRR-E-CONST-PANIC", plan.message()), at);
                failure.panic = Some(Box::new(ConstPanic {
                    message: plan.message().into(),
                    definition: plan.definition(),
                    proof: plan.identity(),
                }));
                return Err(failure);
            }
            Node::Panic => {
                return Err(self.failure(
                    fault("LRR-E-CONST-PANIC", "reached explicit constant panic"),
                    at,
                ));
            }
            Node::OutsideDomain(what) => {
                return Err(self.failure(crate::syntax::pure_constants::outside_domain(what), at));
            }
            Node::Unsupported => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-UNSUPPORTED",
                        "reached operation is outside this pure constant slice",
                    ),
                    at,
                ));
            }
            _ => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-SOURCE",
                        "constant node family dispatch differs",
                    ),
                    at,
                ));
            }
        };
        let value = self.checked_value(value, at)?;
        if let Some(expected) = hint {
            self.check_expected(&value, expected, at)?;
        }
        Ok(Flow::Value(value))
    }
    fn local_context(&mut self, statement: &StatementRef) -> Result<PrimitiveType, ConstFailure> {
        let at = statement.range();
        let key = (at.start, at.end);
        if let Some(primitive) = self.local_contexts.get(&key) {
            return Ok(*primitive);
        }
        let remaining_work = usize::MAX - self.local_context_work;
        let remaining_bytes = usize::MAX - self.metadata_bytes;
        let context = self
            .source
            .numeric_local_context(statement, remaining_work, remaining_bytes)
            .map_err(|d| self.failure(d, at))?;
        self.local_context_work = self
            .local_context_work
            .checked_add(context.work())
            .ok_or_else(|| self.limit(at))?;
        self.admit_metadata(
            context
                .metadata_bytes()
                .checked_add(128)
                .ok_or_else(|| self.limit(at))?,
            at,
        )?;
        let primitive = context.primitive();
        self.local_contexts.insert(key, primitive);
        Ok(primitive)
    }
    #[inline(never)]
    fn run_block(
        &mut self,
        statements: Vec<StatementRef>,
        tail: Option<ExprRef>,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        at: SourceRange,
    ) -> Result<Flow, ConstFailure> {
        self.admit_metadata(std::mem::size_of::<LocalScopes>() + 64, at)?;
        locals.push().map_err(|d| self.failure(d, at))?;
        let result = self.run_block_contents(statements, tail, hint, locals, allow_return, at);
        // Pop on every flow and failure; writes in older scopes survive.
        locals.pop().map_err(|d| self.failure(d, at))?;
        match result {
            Ok(Flow::Value(value)) => {
                if let Some(expected) = hint {
                    self.check_expected(&value, expected, at)?;
                }
                Ok(Flow::Value(self.checked_value(value, at)?))
            }
            other => other,
        }
    }
    #[inline(never)]
    fn run_block_contents(
        &mut self,
        statements: Vec<StatementRef>,
        tail: Option<ExprRef>,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
        at: SourceRange,
    ) -> Result<Flow, ConstFailure> {
        for statement in statements {
            match self.run_statement(&statement, locals, allow_return)? {
                Flow::Value(_) => {}
                other => return Ok(other),
            }
        }
        if let Some(tail) = tail {
            self.expr(&tail, hint, locals, allow_return)
        } else {
            Ok(Flow::Value(
                ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?,
            ))
        }
    }
    #[inline(never)]
    fn run_statement(
        &mut self,
        statement: &StatementRef,
        locals: &mut LocalScopes,
        allow_return: bool,
    ) -> Result<Flow, ConstFailure> {
        self.steps = self
            .steps
            .checked_add(1)
            .ok_or_else(|| self.limit(statement.range()))?;
        match self
            .source
            .statement(statement)
            .map_err(|d| self.failure(d, statement.range()))?
        {
            Statement::Let(name, mutable, ty, initializer) => {
                let ty = match ty {
                    Some(ty) => Some(ty),
                    None if self.hint(&initializer, locals, 0)?.is_some() => None,
                    None => Some(self.local_context(statement)?),
                };
                let value = take!(
                    self,
                    &initializer,
                    ty.map(Expected::Scalar),
                    locals,
                    allow_return
                );
                self.admit_metadata(
                    name.len()
                        .checked_add(192)
                        .ok_or_else(|| self.limit(initializer.range()))?,
                    initializer.range(),
                )?;
                locals
                    .insert(name, mutable, value)
                    .map_err(|d| self.failure(d, initializer.range()))?;
            }
            Statement::Expression(child) => {
                take!(self, &child, None, locals, allow_return);
            }
            Statement::Declaration => {}
            Statement::Unsupported(at) => {
                return Err(self.failure(
                    fault(
                        "LRR-E-CONST-UNSUPPORTED",
                        "reached constant statement requires a later pure slice",
                    ),
                    at,
                ));
            }
        }
        Ok(Flow::Value(
            ConstValue::from_scalar(ValueRepr::unit())
                .map_err(|d| self.failure(d, statement.range()))?,
        ))
    }
    fn activate_control(
        &mut self,
        target: ControlTarget,
        expected: Option<Expected>,
        at: SourceRange,
    ) -> Result<(), ConstFailure> {
        self.admit_metadata(
            std::mem::size_of::<(ControlTarget, Option<Expected>)>() + 64,
            at,
        )?;
        self.loops.try_reserve(1).map_err(|_| self.limit(at))?;
        self.loops.push((target, expected));
        Ok(())
    }
    fn run_loop(
        &mut self,
        target: ControlTarget,
        condition: Option<ExprRef>,
        body: ExprRef,
        hint: Option<Expected>,
        locals: &mut LocalScopes,
        allow_return: bool,
    ) -> Result<Flow, ConstFailure> {
        let unit = Some(Expected::Scalar(PrimitiveType::Unit));
        let expected = if condition.is_some() { unit } else { hint };
        self.activate_control(target, expected, body.range())?;
        let result = (|| loop {
            if let Some(condition) = &condition {
                match self.expr(
                    condition,
                    Some(Expected::Scalar(PrimitiveType::Bool)),
                    locals,
                    allow_return,
                )? {
                    Flow::Value(value) => {
                        let value =
                            scalar(value).map_err(|d| self.failure(d, condition.range()))?;
                        if value.type_id != 2 {
                            return Err(self.failure(
                                fault("LRR-E-CONST-TYPE", "constant while condition requires bool"),
                                condition.range(),
                            ));
                        }
                        if value.payload == 0 {
                            return Ok(Flow::Value(
                                ConstValue::from_scalar(ValueRepr::unit())
                                    .map_err(|d| self.failure(d, condition.range()))?,
                            ));
                        }
                    }
                    Flow::Continue(current) if current == target => continue,
                    Flow::Break(current, value) if current == target => {
                        if let Some(expected) = expected {
                            self.check_expected(&value, expected, condition.range())?;
                        }
                        return Ok(Flow::Value(value));
                    }
                    other => return Ok(other),
                }
            }
            match self.expr(&body, unit, locals, allow_return)? {
                Flow::Value(_) => {}
                Flow::Continue(current) if current == target => {}
                Flow::Break(current, value) if current == target => {
                    if let Some(expected) = expected {
                        self.check_expected(&value, expected, body.range())?;
                    }
                    return Ok(Flow::Value(value));
                }
                other => return Ok(other),
            }
        })();
        self.loops.pop();
        match result {
            Ok(Flow::Value(value)) => {
                if let Some(expected) = hint {
                    self.check_expected(&value, expected, body.range())?;
                }
                Ok(Flow::Value(self.checked_value(value, body.range())?))
            }
            other => other,
        }
    }
    fn check_expected(
        &self,
        value: &ConstValue,
        expected: Expected,
        at: SourceRange,
    ) -> Result<(), ConstFailure> {
        let matches = match expected {
            Expected::Scalar(p) => value.matches_scalar(p).map_err(|d| self.failure(d, at))?,
            Expected::Array(p, len) => value.elements().is_some_and(|e| {
                e.len() == len && e.iter().all(|v| v.matches_scalar(p).unwrap_or(false))
            }),
        };
        if !matches {
            return Err(self.failure(
                fault(
                    "LRR-E-CONST-TYPE",
                    "constant value differs from reached concrete type context",
                ),
                at,
            ));
        }
        Ok(())
    }
    fn array_admission(
        &self,
        count: usize,
        element_bytes: usize,
        at: SourceRange,
    ) -> Result<(), ConstFailure> {
        let bytes = count
            .checked_mul(element_bytes)
            .and_then(|n| n.checked_add(9 + 8 + ConstValue::DOMAIN.len()))
            .ok_or_else(|| self.limit(at))?;
        count
            .checked_mul(std::mem::size_of::<ConstValue>())
            .ok_or_else(|| self.limit(at))?;
        let _ = bytes;
        Ok(())
    }
}
fn comparison(op: NumericBinary) -> bool {
    matches!(
        op,
        NumericBinary::Eq
            | NumericBinary::Ne
            | NumericBinary::Lt
            | NumericBinary::Le
            | NumericBinary::Gt
            | NumericBinary::Ge
    )
}
fn primitive_for_value(v: Option<ValueRepr>) -> Option<PrimitiveType> {
    use PrimitiveType as P;
    Some(match v?.type_id {
        1 => P::Unit,
        2 => P::Bool,
        3 => P::Char,
        4 => P::I8,
        5 => P::I16,
        6 => P::I32,
        7 => P::I64,
        8 => P::Isize,
        9 => P::U8,
        10 => P::U16,
        11 => P::U32,
        12 => P::U64,
        13 => P::Usize,
        14 => P::F32,
        15 => P::F64,
        _ => return None,
    })
}
/// Returns an immutable result or an attributed typed refusal. This is not query publication.
pub fn evaluate(plan: &ConstantPlan, limits: ConstLimits) -> Result<ConstEvaluation, ConstFailure> {
    evaluate_inner(plan, limits).map_err(|mut failure| {
        failure.requested_plan = Some(plan.identity());
        failure
    })
}
fn evaluate_inner(
    plan: &ConstantPlan,
    limits: ConstLimits,
) -> Result<ConstEvaluation, ConstFailure> {
    let mut evaluator = Evaluator {
        source: plan.source(),
        limits,
        steps: 0,
        frames: 0,
        depth: 0,
        returns: Vec::new(),
        loops: Vec::new(),
        metadata_bytes: 0,
        active: BTreeSet::new(),
        active_calls: BTreeSet::new(),
        dependencies: BTreeMap::new(),
        nodes: BTreeMap::new(),
        local_contexts: BTreeMap::new(),
        local_context_work: 0,
    };
    let (constant, call, length) = plan
        .entry()
        .map_err(|d| evaluator.failure(d, plan.range()))?;
    let (value, expected) = if let Some(c) = constant {
        evaluator.constant(c)?
    } else if let Some((f, args)) = call {
        evaluator.function(f, args, plan.range())?
    } else if let Some(length) = length {
        let value = evaluator
            .expr(
                &length,
                Some(Expected::Scalar(PrimitiveType::Usize)),
                &mut LocalScopes::default(),
                false,
            )?
            .into_value()
            .map_err(|d| evaluator.failure(d, length.range()))?;
        evaluator.length(value.clone(), length.range())?;
        (value, Expected::Scalar(PrimitiveType::Usize))
    } else {
        return Err(evaluator.failure(
            fault("LRR-E-CONST-SOURCE", "sealed constant entry absent"),
            plan.range(),
        ));
    };
    let value = evaluator.checked_value(value, plan.range())?;
    let value_type = match expected {
        Expected::Scalar(ty) => wire::ConstantWireType::Scalar(ty),
        Expected::Array(ty, length) => wire::ConstantWireType::Array {
            element: Box::new(wire::ConstantWireType::Scalar(ty)),
            length: u64::try_from(length).map_err(|_| evaluator.limit(plan.range()))?,
        },
    };
    Ok(ConstEvaluation {
        value,
        value_type,
        source: plan.source().source(),
        cfg: plan.source().cfg(),
        plan: plan.identity(),
        dependencies: evaluator.dependencies.into_values().collect(),
        steps: evaluator.steps,
    })
}

pub mod memo;
pub mod query_identity;
pub mod wire;

pub mod query;
pub mod query_value;

pub mod query_failure;
pub mod query_macro_demand;

#[cfg(test)]
#[path = "constant_loop_tests.rs"]
mod loop_tests;

#[cfg(test)]
#[path = "constant_literal_family_tests.rs"]
mod literal_family_tests;
