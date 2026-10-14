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
            ConstType, Constant, ConstantPlan, ConstantSource, ExprRef, Function, Node, Statement,
        },
        spans::SourceRange,
    },
    vm::numeric::{self, NumericFailure},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const MAX_STEPS: u32 = 100_000;
const MAX_FRAMES: u16 = 128;
const MAX_BYTES: usize = 1 << 20;
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Debug, Clone)]
enum Immutable {
    Scalar(ValueRepr),
    Array(Arc<[ConstValue]>),
}
/// Fields are private: no guest/object/reference/function handle can enter a constant value.
#[derive(Debug, Clone)]
pub struct ConstValue(Immutable);
impl ConstValue {
    pub fn from_scalar(value: ValueRepr) -> Result<Self, Diagnostic> {
        if !matches!(value.kind, 1..=7) {
            return Err(fault(
                "LRR-E-CONST-TYPE",
                "constant scalar cannot contain a guest handle",
            ));
        }
        value.validate_scalar()?;
        Ok(Self(Immutable::Scalar(value)))
    }
    pub fn scalar(&self) -> Option<ValueRepr> {
        match &self.0 {
            Immutable::Scalar(v) => Some(*v),
            _ => None,
        }
    }
    pub fn elements(&self) -> Option<&[ConstValue]> {
        match &self.0 {
            Immutable::Array(es) => Some(es),
            _ => None,
        }
    }
    pub(crate) fn matches_scalar(&self, ty: PrimitiveType) -> Result<bool, Diagnostic> {
        Ok(self.scalar().is_some_and(|v| {
            Some(v.type_id) == PrimitiveIdentity::from_recipe(ty).ok().map(|id| id.id().0)
        }))
    }
    fn field_bytes(&self) -> Option<usize> {
        match &self.0 {
            Immutable::Scalar(_) => Some(17),
            Immutable::Array(es) => es
                .iter()
                .try_fold(9usize, |n, e| n.checked_add(e.field_bytes()?)),
        }
    }
    fn serialized_bytes(&self) -> Option<usize> {
        self.field_bytes()?
            .checked_add(8)?
            .checked_add(Self::DOMAIN.len())
    }
}
impl StableEncode for ConstValue {
    const DOMAIN: &'static str = "lrr/v1/immutable-constant-value";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match &self.0 {
            Immutable::Scalar(v) => {
                e.u8(1);
                e.u32(v.kind);
                e.u32(v.type_id);
                e.u64(v.payload)
            }
            Immutable::Array(es) => {
                e.u8(2);
                e.sequence(es, |e, v| v.encode_fields(e))
            }
        }
    }
}
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
            result_bytes: MAX_BYTES,
        }
    }
}
impl ConstLimits {
    pub fn new(steps: u32, frames: u16, result_bytes: usize) -> Result<Self, Diagnostic> {
        if steps == 0
            || steps > MAX_STEPS
            || frames == 0
            || frames > MAX_FRAMES
            || result_bytes == 0
            || result_bytes > MAX_BYTES
        {
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
    /// Pure admission calculation; the runtime must supply its actual element layout/limits.
    pub fn admit_array_length(
        &self,
        element_bytes: usize,
        max_elements: usize,
        max_allocation_bytes: usize,
    ) -> Result<usize, Diagnostic> {
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
        if count > max_elements
            || count
                .checked_mul(element_bytes)
                .is_none_or(|n| n > max_allocation_bytes)
        {
            return Err(fault(
                "LRR-E-CONST-LIMIT",
                "array length exceeds element/allocation limits",
            ));
        }
        Ok(count)
    }
}
#[derive(Debug)]
pub struct ConstFailure {
    diagnostic: Box<Diagnostic>,
    source: Digest,
    range: SourceRange,
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
}
#[derive(Debug)]
enum Flow {
    Value(ConstValue),
    Return(ConstValue),
}
impl Flow {
    fn into_value(self) -> ConstValue {
        match self {
            Self::Value(v) | Self::Return(v) => v,
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
    metadata_bytes: usize,
    active: BTreeSet<Digest>,
    active_calls: BTreeSet<Digest>,
    dependencies: BTreeMap<Digest, ConstDependency>,
    nodes: BTreeMap<(u32, u32), Node>,
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
        at: SourceRange,
    ) -> Result<ConstValue, ConstFailure> {
        if value
            .serialized_bytes()
            .is_none_or(|n| n > self.limits.result_bytes)
        {
            return Err(self.limit(at));
        }
        Ok(value)
    }
    fn node(&mut self, expr: &ExprRef) -> Result<Node, ConstFailure> {
        if let Some(n) = self.nodes.get(&expr.key()) {
            let bytes = n.owned_bytes().ok_or_else(|| self.limit(expr.range()))?;
            self.metadata_bytes = self
                .metadata_bytes
                .checked_add(bytes)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| self.limit(expr.range()))?;
            return Ok(n.clone());
        }
        let n = self
            .source
            .node(expr)
            .map_err(|d| self.failure(d, expr.range()))?;
        if self.nodes.len() >= MAX_STEPS as usize {
            return Err(self.limit(expr.range()));
        }
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
        let v = result.map_err(|e| {
            self.failure(
                match e {
                    NumericFailure::GuestPanic(p) => fault("LRR-E-CONST-PANIC", p.description()),
                    NumericFailure::RuntimeFault(d) => d,
                },
                at,
            )
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
                        &mut BTreeMap::new(),
                        false,
                    )?
                    .into_value();
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
            .filter(|n| *n <= 32 * 1024 * 1024)
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
        let value = (|| {
            let expected = self.ty(&c.ty, at)?;
            let value = self
                .expr(&c.body, Some(expected), &mut BTreeMap::new(), false)?
                .into_value();
            self.check_expected(&value, expected, at)?;
            Ok((value, expected))
        })();
        self.depth = previous_depth;
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
            .try_fold(0usize, |n, (name, _)| {
                n.checked_add(name.len())?.checked_add(192)
            })
            .ok_or_else(|| self.limit(at))?;
        self.admit_metadata(bytes, at)?;
        let mut locals = BTreeMap::new();
        for ((name, ty), value) in f.params.iter().zip(&args) {
            if !value.matches_scalar(*ty).map_err(|d| self.failure(d, at))? {
                return Err(self.failure(
                    fault("LRR-E-CONST-TYPE", "const-call argument type differs"),
                    at,
                ));
            }
            locals.insert(name.clone(), value.clone());
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
        let value = (|| {
            let expected = self.ty(&f.result, at)?;
            self.returns.push(expected);
            let value = self.expr(&f.body, Some(expected), &mut locals, true);
            self.returns.pop();
            let value = value?.into_value();
            self.check_expected(&value, expected, at)?;
            Ok((value, expected))
        })();
        self.depth = previous_depth;
        self.frames -= 1;
        self.active_calls.remove(&call_key);
        value
    }
    fn hint(
        &mut self,
        e: &ExprRef,
        locals: &BTreeMap<String, ConstValue>,
        depth: u16,
    ) -> Result<Option<PrimitiveType>, ConstFailure> {
        if depth >= 128 {
            return Err(self.limit(e.range()));
        }
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
    fn expr(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut BTreeMap<String, ConstValue>,
        allow_return: bool,
    ) -> Result<Flow, ConstFailure> {
        self.steps = self
            .steps
            .checked_add(1)
            .filter(|s| *s <= self.limits.steps)
            .ok_or_else(|| self.limit(e.range()))?;
        if self.depth >= 128 {
            return Err(self.limit(e.range()));
        }
        self.depth += 1;
        let result = self.inner(e, hint, locals, allow_return);
        self.depth -= 1;
        result
    }
    fn inner(
        &mut self,
        e: &ExprRef,
        hint: Option<Expected>,
        locals: &mut BTreeMap<String, ConstValue>,
        allow_return: bool,
    ) -> Result<Flow, ConstFailure> {
        let at = e.range();
        let scalar_hint = match hint {
            Some(Expected::Scalar(p)) => Some(p),
            _ => None,
        };
        let value = match self.node(e)? {
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
                ConstValue::from_scalar(
                    GuestStore::preview_float(
                        &text,
                        primitive_id(p).map_err(|d| self.failure(d, at))?,
                    )
                    .map_err(|d| self.failure(d, at))?,
                )
                .map_err(|d| self.failure(d, at))?
            }
            Node::Parentheses(child) => return self.expr(&child, hint, locals, allow_return),
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
                let proven = self.hint(&child, locals, 0)?.ok_or_else(|| {
                    self.failure(
                        fault(
                            "LRR-E-CONST-TYPE",
                            "unsuffixed numeric cast requires source context integration",
                        ),
                        at,
                    )
                })?;
                let v = take!(
                    self,
                    &child,
                    Some(Expected::Scalar(proven)),
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
            Node::Block(statements, tail) => {
                let bytes = locals
                    .keys()
                    .try_fold(0usize, |n, name| {
                        n.checked_add(name.len())?.checked_add(192)
                    })
                    .ok_or_else(|| self.limit(at))?;
                self.admit_metadata(bytes, at)?;
                let mut block = locals.clone();
                for statement in statements {
                    self.steps = self
                        .steps
                        .checked_add(1)
                        .filter(|s| *s <= self.limits.steps)
                        .ok_or_else(|| self.limit(statement.range()))?;
                    match self
                        .source
                        .statement(&statement)
                        .map_err(|d| self.failure(d, statement.range()))?
                    {
                        Statement::Let(name, p, initializer) => {
                            if p.is_none() && self.hint(&initializer, &block, 0)?.is_none() {
                                return Err(self.failure(fault("LRR-E-CONST-UNSUPPORTED","unannotated numeric local requires source constraint integration"),initializer.range()));
                            }
                            let v = take!(
                                self,
                                &initializer,
                                p.map(Expected::Scalar),
                                &mut block,
                                allow_return
                            );
                            if !block.contains_key(&name) {
                                if block.len() >= 256 {
                                    return Err(self.limit(initializer.range()));
                                }
                                self.admit_metadata(
                                    name.len()
                                        .checked_add(192)
                                        .ok_or_else(|| self.limit(initializer.range()))?,
                                    initializer.range(),
                                )?;
                            }
                            block.insert(name, v);
                        }
                        Statement::Expression(child) => {
                            take!(self, &child, None, &mut block, allow_return);
                        }
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
                }
                if let Some(tail) = tail {
                    return self.expr(&tail, hint, &mut block, allow_return);
                }
                ConstValue::from_scalar(ValueRepr::unit()).map_err(|d| self.failure(d, at))?
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
                for (arg, (_, ty)) in args.iter().zip(&f.params) {
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
                ConstValue(Immutable::Array(values.into()))
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
                ConstValue(Immutable::Array(values.into()))
            }
            Node::Panic => {
                return Err(self.failure(
                    fault("LRR-E-CONST-PANIC", "reached explicit constant panic"),
                    at,
                ));
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
        };
        let value = self.checked_value(value, at)?;
        if let Some(expected) = hint {
            self.check_expected(&value, expected, at)?
        }
        Ok(Flow::Value(value))
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
        if bytes > self.limits.result_bytes {
            return Err(self.limit(at));
        }
        count
            .checked_mul(std::mem::size_of::<ConstValue>())
            .filter(|n| *n <= MAX_BYTES * 4)
            .ok_or_else(|| self.limit(at))?;
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
    let mut evaluator = Evaluator {
        source: plan.source(),
        limits,
        steps: 0,
        frames: 0,
        depth: 0,
        returns: Vec::new(),
        metadata_bytes: 0,
        active: BTreeSet::new(),
        active_calls: BTreeSet::new(),
        dependencies: BTreeMap::new(),
        nodes: BTreeMap::new(),
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
                &mut BTreeMap::new(),
                false,
            )?
            .into_value();
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
pub mod wire;
