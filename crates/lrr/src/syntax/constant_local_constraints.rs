//! Bounded scalar equations for one source-owned constant-local demand.
//! AST handles, lexical binding IDs and graph nodes never leave this adapter.
use super::*;
#[path = "constant_constraint_work.rs"]
mod source_work;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Integer,
    Float,
}
fn family(ty: PrimitiveType) -> Option<Family> {
    use PrimitiveType as P;
    match ty {
        P::I8
        | P::I16
        | P::I32
        | P::I64
        | P::Isize
        | P::U8
        | P::U16
        | P::U32
        | P::U64
        | P::Usize => Some(Family::Integer),
        P::F32 | P::F64 => Some(Family::Float),
        _ => None,
    }
}
// Source contexts are hints, not eager type errors. Preserve the first
// compatible context (including an earlier owning result), while retaining an
// incompatible fact only when neither source supplies a compatible one. The
// final family filter and actual reached evaluator checks remain authoritative.
fn preferred_context(
    first: Option<PrimitiveType>,
    next: Option<PrimitiveType>,
    expected: Option<Family>,
) -> Option<PrimitiveType> {
    let compatible = |ty: &PrimitiveType| expected.is_none_or(|f| family(*ty) == Some(f));
    first
        .filter(compatible)
        .or(next.filter(compatible))
        .or(first)
        .or(next)
}
#[derive(Clone, Copy)]
struct Constraint {
    parent: usize,
    concrete: Option<PrimitiveType>,
    family: Option<Family>,
    opaque: bool,
}
struct Graph<'a> {
    source: &'a ConstantSource,
    nodes: Vec<Constraint>,
    scopes: Vec<BTreeMap<String, usize>>,
    loops: Vec<(ControlTarget, usize)>,
    result: usize,
    target: SourceRange,
    selected: Option<usize>,
    work: usize,
    bytes: usize,
    work_limit: usize,
    byte_limit: usize,
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-CONST-LIMIT",
        "constant local constraint budget".into(),
    )
}
fn incomplete() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-CONST-UNSUPPORTED",
        "constant local has unresolved source type constraints".into(),
    )
}
impl Graph<'_> {
    fn charge(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.work = self
            .work
            .checked_add(1)
            .filter(|n| *n <= self.work_limit)
            .ok_or_else(limit)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.byte_limit)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn fresh(&mut self) -> Result<usize, Diagnostic> {
        self.charge(std::mem::size_of::<Constraint>() + 64)?;
        self.nodes.try_reserve(1).map_err(|_| limit())?;
        let id = self.nodes.len();
        self.nodes.push(Constraint {
            parent: id,
            concrete: None,
            family: None,
            opaque: false,
        });
        Ok(id)
    }
    fn root(&mut self, id: usize) -> Result<usize, Diagnostic> {
        let mut at = id;
        loop {
            self.charge(0)?;
            let node = self.nodes.get(at).ok_or_else(incomplete)?;
            if node.parent == at {
                break;
            }
            at = node.parent;
        }
        let root = at;
        at = id;
        while self.nodes[at].parent != at {
            self.charge(0)?;
            let next = self.nodes[at].parent;
            self.nodes[at].parent = root;
            at = next;
        }
        Ok(root)
    }
    fn combine(&mut self, a: usize, b: usize) -> Result<usize, Diagnostic> {
        let a = self.root(a)?;
        let b = self.root(b)?;
        if a == b {
            return Ok(a);
        }
        let (keep, remove) = if a < b { (a, b) } else { (b, a) };
        let x = self.nodes[keep];
        let y = self.nodes[remove];
        let family = x.family.or(y.family);
        let concrete = preferred_context(x.concrete, y.concrete, family);
        self.nodes[keep] = Constraint {
            parent: keep,
            concrete,
            family,
            opaque: x.opaque || y.opaque,
        };
        self.nodes[remove].parent = keep;
        Ok(keep)
    }
    fn known(&mut self, id: usize, ty: PrimitiveType) -> Result<(), Diagnostic> {
        let id = self.root(id)?;
        let node = &mut self.nodes[id];
        // These are source-derived contextual hints, not acceptance of every
        // expression in the owner. Keep the first family-compatible context on
        // this deterministic representative; the earlier owning result retains
        // priority. An incompatible cold hint cannot suppress that context.
        node.concrete = preferred_context(node.concrete, Some(ty), node.family);
        Ok(())
    }
    fn numeric(&mut self, id: usize, f: Family) -> Result<(), Diagnostic> {
        let id = self.root(id)?;
        let node = &mut self.nodes[id];
        node.family.get_or_insert(f);
        Ok(())
    }
    fn opaque(&mut self, id: usize) -> Result<(), Diagnostic> {
        let id = self.root(id)?;
        self.nodes[id].opaque = true;
        Ok(())
    }
    fn opaque_scope(&mut self) -> Result<(), Diagnostic> {
        let count = self
            .scopes
            .iter()
            .try_fold(0usize, |n, scope| n.checked_add(scope.len()))
            .ok_or_else(limit)?;
        self.charge(
            count
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(limit)?,
        )?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(count).map_err(|_| limit())?;
        for scope in &self.scopes {
            ids.extend(scope.values().copied());
        }
        for id in ids {
            self.opaque(id)?;
        }
        Ok(())
    }
    fn push(&mut self) -> Result<(), Diagnostic> {
        if self.scopes.len() >= 128 {
            return Err(limit());
        }
        self.charge(64)?;
        self.scopes.try_reserve(1).map_err(|_| limit())?;
        self.scopes.push(BTreeMap::new());
        Ok(())
    }
    fn name(&mut self, text: &str) -> Result<String, Diagnostic> {
        if text.len() > 64 * 1024 {
            return Err(limit());
        }
        self.charge(
            text.len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(64))
                .ok_or_else(limit)?,
        )?;
        semantic(text)
    }
    fn bind(&mut self, name: String, id: usize) -> Result<(), Diagnostic> {
        self.charge(name.capacity().checked_add(64).ok_or_else(limit)?)?;
        self.scopes
            .last_mut()
            .ok_or_else(incomplete)?
            .insert(name, id);
        Ok(())
    }
    fn local(&mut self, name: &str) -> Result<Option<usize>, Diagnostic> {
        for index in (0..self.scopes.len()).rev() {
            self.charge(0)?;
            if let Some(id) = self.scopes[index].get(name).copied() {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
    fn block(&mut self, block: ast::BlockExpr, depth: usize) -> Result<Option<usize>, Diagnostic> {
        if block.async_token().is_some()
            || block.const_token().is_some()
            || block.gen_token().is_some()
            || block.unsafe_token().is_some()
            || block.try_block_modifier().is_some()
        {
            self.opaque_scope()?;
            let id = self.fresh()?;
            self.opaque(id)?;
            return Ok(Some(id));
        }
        let list = required(block.stmt_list())?;
        let labelled = if block.label().is_some() {
            self.charge(std::mem::size_of::<(ControlTarget, usize)>() + 64)?;
            let id = self.fresh()?;
            self.loops.try_reserve(1).map_err(|_| limit())?;
            self.loops
                .push((ControlTarget::block_expression(&block), id));
            Some(id)
        } else {
            None
        };
        self.push()?;
        let result = (|| {
            let mut diverges = false;
            for statement in list.statements() {
                self.charge(0)?;
                match statement {
                    ast::Stmt::LetStmt(binding) => {
                        let ast::Pat::IdentPat(pattern) = required(binding.pat())? else {
                            self.opaque_scope()?;
                            continue;
                        };
                        let id = self.fresh()?;
                        if !self.checked_attributes(binding.syntax())?
                            || binding.let_else().is_some()
                            || pattern.ref_token().is_some()
                            || pattern.at_token().is_some()
                        {
                            self.opaque(id)?;
                            self.opaque_scope()?;
                        }
                        let initializer = match binding.initializer() {
                            Some(expr) => self.expr(expr, depth + 1)?,
                            None => {
                                self.opaque(id)?;
                                None
                            }
                        };
                        if let Some(initializer) = initializer {
                            self.combine(id, initializer)?;
                        } else if binding.initializer().is_some() {
                            diverges = true;
                        }
                        if let Some(ty) = binding.ty() {
                            match self.checked_primitive(ty)? {
                                Some(ty) => self.known(id, ty)?,
                                None => self.opaque(id)?,
                            }
                        }
                        let name = self.name(required(pattern.name())?.text())?;
                        self.bind(name, id)?;
                        if range(binding.syntax().text_range()) == self.target {
                            self.selected = Some(id);
                        }
                    }
                    ast::Stmt::ExprStmt(statement) => {
                        if let Some(expr) = statement.expr() {
                            diverges |= self.expr(expr, depth + 1)?.is_none();
                        }
                    }
                    // A local item body cannot capture these function locals.
                    ast::Stmt::Item(ast::Item::MacroCall(_) | ast::Item::Use(_)) => {
                        self.opaque_scope()?
                    }
                    ast::Stmt::Item(_) => {}
                }
            }
            let tail = match list.tail_expr() {
                Some(tail) => self.expr(tail, depth + 1)?,
                None => {
                    let id = self.fresh()?;
                    self.known(id, PrimitiveType::Unit)?;
                    Some(id)
                }
            };
            Ok(if diverges { None } else { tail })
        })();
        self.scopes.pop().ok_or_else(incomplete)?;
        if let Some(id) = labelled {
            self.loops.pop().ok_or_else(incomplete)?;
            if let Some(tail) = result? {
                self.combine(id, tail)?;
            }
            Ok(Some(id))
        } else {
            result
        }
    }
    fn expr(&mut self, expr: ast::Expr, depth: usize) -> Result<Option<usize>, Diagnostic> {
        self.charge(0)?;
        if depth >= 128 {
            return Err(limit());
        }
        let id = self.fresh()?;
        if !self.checked_attributes(expr.syntax())? {
            self.opaque_scope()?;
            self.opaque(id)?;
        }
        use ast::Expr as E;
        match expr.clone() {
            E::Literal(_) | E::TupleExpr(_) => match self.checked_literal(expr)? {
                Some(Node::Integer(DecodedLiteral::Integer { suffix, .. })) => {
                    self.numeric(id, Family::Integer)?;
                    if let Some(ty) = suffix {
                        self.known(id, ty)?;
                    }
                }
                Some(Node::Float(_, suffix)) => {
                    self.numeric(id, Family::Float)?;
                    if let Some(ty) = suffix {
                        self.known(id, ty)?;
                    }
                }
                Some(Node::Bool(_)) => self.known(id, PrimitiveType::Bool)?,
                Some(Node::Char(_)) => self.known(id, PrimitiveType::Char)?,
                Some(Node::Byte(_)) => self.known(id, PrimitiveType::U8)?,
                Some(Node::Unit) => self.known(id, PrimitiveType::Unit)?,
                _ => self.opaque(id)?,
            },
            E::PathExpr(path) => {
                let name = match path.path() {
                    Some(path) => self.checked_path(path)?,
                    None => None,
                };
                let name = match name {
                    Some(n) => n,
                    None => {
                        self.opaque(id)?;
                        return Ok(Some(id));
                    }
                };
                if !self.checked_lexical_value(path.syntax(), &name)? {
                    self.opaque(id)?;
                }
                if let Some(local) = self.local(&name)? {
                    self.combine(id, local)?;
                } else {
                    match self.checked_named_constant(&name)?.map(|c| c.ty) {
                        Some(ConstType::Scalar(ty)) => self.known(id, ty)?,
                        _ => self.opaque(id)?,
                    }
                }
            }
            E::ParenExpr(group) => return self.expr(required(group.expr())?, depth + 1),
            E::BlockExpr(block) => return self.block(block, depth + 1),
            E::PrefixExpr(prefix) => {
                if !matches!(
                    prefix.op_kind(),
                    Some(ast::UnaryOp::Neg | ast::UnaryOp::Not)
                ) {
                    self.opaque_scope()?;
                    self.opaque(id)?;
                } else if let Some(child) = self.expr(required(prefix.expr())?, depth + 1)? {
                    self.combine(id, child)?;
                } else {
                    return Ok(None);
                }
            }
            E::CastExpr(cast) => {
                self.expr(required(cast.expr())?, depth + 1)?;
                match self.checked_primitive(required(cast.ty())?)? {
                    Some(ty) => self.known(id, ty)?,
                    None => self.opaque(id)?,
                }
            }
            E::BinExpr(binary) => {
                let lhs = self.expr(required(binary.lhs())?, depth + 1)?;
                let rhs = self.expr(required(binary.rhs())?, depth + 1)?;
                match required(binary.op_kind())? {
                    ast::BinaryOp::LogicOp(_) => {
                        self.known(id, PrimitiveType::Bool)?;
                        for child in [lhs, rhs].into_iter().flatten() {
                            self.known(child, PrimitiveType::Bool)?;
                        }
                    }
                    ast::BinaryOp::Assignment { op } => {
                        self.known(id, PrimitiveType::Unit)?;
                        if let (Some(a), Some(b)) = (lhs, rhs) {
                            if matches!(op, Some(ast::ArithOp::Shl | ast::ArithOp::Shr)) {
                                self.numeric(b, Family::Integer)?;
                            } else {
                                self.combine(a, b)?;
                            }
                        }
                    }
                    ast::BinaryOp::CmpOp(_) => {
                        self.known(id, PrimitiveType::Bool)?;
                        if let (Some(a), Some(b)) = (lhs, rhs) {
                            self.combine(a, b)?;
                        }
                    }
                    ast::BinaryOp::ArithOp(op) => {
                        if let Some(a) = lhs {
                            self.combine(id, a)?;
                        }
                        if let Some(b) = rhs {
                            if matches!(op, ast::ArithOp::Shl | ast::ArithOp::Shr) {
                                self.numeric(b, Family::Integer)?;
                            } else {
                                self.combine(id, b)?;
                            }
                        }
                    }
                }
                if lhs.is_none() || rhs.is_none() {
                    return Ok(None);
                }
            }
            E::IfExpr(branch) => {
                if let Some(condition) = self.expr(required(branch.condition())?, depth + 1)? {
                    self.known(condition, PrimitiveType::Bool)?;
                }
                let yes = self.block(required(branch.then_branch())?, depth + 1)?;
                let no = match branch.else_branch() {
                    Some(ast::ElseBranch::Block(b)) => self.block(b, depth + 1)?,
                    Some(ast::ElseBranch::IfExpr(i)) => self.expr(E::IfExpr(i), depth + 1)?,
                    None => {
                        let unit = self.fresh()?;
                        self.known(unit, PrimitiveType::Unit)?;
                        Some(unit)
                    }
                };
                for child in [yes, no].into_iter().flatten() {
                    self.combine(id, child)?;
                }
                if yes.is_none() && no.is_none() {
                    return Ok(None);
                }
            }
            E::LoopExpr(loop_expr) => {
                self.loops.try_reserve(1).map_err(|_| limit())?;
                self.loops
                    .push((ControlTarget::loop_expression(&ExprRef(expr)), id));
                self.block(required(loop_expr.loop_body())?, depth + 1)?;
                self.loops.pop().ok_or_else(incomplete)?;
            }
            E::WhileExpr(loop_expr) => {
                self.known(id, PrimitiveType::Unit)?;
                self.loops.try_reserve(1).map_err(|_| limit())?;
                self.loops
                    .push((ControlTarget::loop_expression(&ExprRef(expr)), id));
                if let Some(condition) = self.expr(required(loop_expr.condition())?, depth + 1)? {
                    self.known(condition, PrimitiveType::Bool)?;
                }
                self.block(required(loop_expr.loop_body())?, depth + 1)?;
                self.loops.pop().ok_or_else(incomplete)?;
            }
            E::BreakExpr(exit) => {
                let Some(target) = self.checked_loop_target(exit.syntax(), exit.lifetime())? else {
                    // This source-context walk cannot validate a cold exit or
                    // invent a target. The actual evaluator still resolves the
                    // real exit if entered, before producing a control effect.
                    self.opaque_scope()?;
                    return Ok(None);
                };
                let destination = self
                    .loops
                    .iter()
                    .rev()
                    .find(|(t, _)| *t == target)
                    .map(|(_, id)| *id)
                    .ok_or_else(incomplete)?;
                if let Some(value) = exit.expr() {
                    if let Some(child) = self.expr(value, depth + 1)? {
                        self.combine(destination, child)?;
                    }
                } else {
                    self.known(destination, PrimitiveType::Unit)?;
                }
                return Ok(None);
            }
            E::ContinueExpr(_) => return Ok(None),
            E::ReturnExpr(exit) => {
                if let Some(value) = exit.expr() {
                    if let Some(child) = self.expr(value, depth + 1)? {
                        self.combine(self.result, child)?;
                    }
                } else {
                    self.known(self.result, PrimitiveType::Unit)?;
                }
                return Ok(None);
            }
            E::CallExpr(call) => {
                let function = if let Some(E::PathExpr(path)) = call.expr() {
                    let name = match path.path() {
                        Some(path) => self.checked_path(path)?,
                        None => None,
                    };
                    match name {
                        Some(name)
                            if self.local(&name)?.is_none()
                                && self.checked_lexical_value(call.syntax(), &name)? =>
                        {
                            self.checked_named_function(&name)?
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                let arguments = required(call.arg_list())?;
                let mut count = 0usize;
                for argument in arguments.args() {
                    self.charge(0)?;
                    let child = self.expr(argument, depth + 1)?;
                    if let Some(child) = child {
                        match function.as_ref().and_then(|f| f.params.get(count)) {
                            Some((_, ty, _)) => self.known(child, *ty)?,
                            None => self.opaque(child)?,
                        }
                    }
                    count = count.checked_add(1).ok_or_else(limit)?;
                }
                match function {
                    Some(function) if function.params.len() == count => match function.result {
                        ConstType::Scalar(ty) => self.known(id, ty)?,
                        _ => self.opaque(id)?,
                    },
                    _ => self.opaque(id)?,
                }
            }
            _ => {
                self.opaque_scope()?;
                self.opaque(id)?;
            }
        }
        Ok(Some(id))
    }
}
pub(super) fn select(
    source: &ConstantSource,
    target: &StatementRef,
    work_limit: usize,
    byte_limit: usize,
) -> Result<LocalNumericContext, Diagnostic> {
    if !(1..=100_000).contains(&work_limit) || !(128..=32 * 1024 * 1024).contains(&byte_limit) {
        return Err(limit());
    }
    let mut graph = Graph {
        source,
        nodes: Vec::new(),
        scopes: Vec::new(),
        loops: Vec::new(),
        result: 0,
        target: target.range(),
        selected: None,
        work: 0,
        bytes: 0,
        work_limit,
        byte_limit,
    };
    graph.checked_target_source(target)?;
    let mut owner = None;
    let mut length = None;
    for node in target.0.syntax().ancestors() {
        graph.charge(0)?;
        let candidate = if let Some(array) = ast::ArrayType::cast(node.clone()) {
            array
                .const_arg()
                .and_then(|argument| argument.syntax().children().find_map(ast::Expr::cast))
        } else if let Some(array) = ast::ArrayExpr::cast(node.clone()) {
            match array.kind() {
                ast::ArrayExprKind::Repeat { repeat, .. } => repeat,
                _ => None,
            }
        } else {
            None
        };
        if let Some(expression) = candidate
            && range(expression.syntax().text_range()).contains(target.range())
        {
            length = Some(ExprRef(expression));
            break;
        }
        if ast::Fn::can_cast(node.kind())
            || ast::Const::can_cast(node.kind())
            || ast::ClosureExpr::can_cast(node.kind())
        {
            owner = Some(node);
            break;
        }
    }
    let (body, params, result) = if let Some(length) = length {
        // usize belongs to the exact admitted length, never to all locals in
        // the surrounding function. The graph starts a fresh lexical scope.
        (
            graph.checked_array_length(length)?,
            Vec::new(),
            ConstType::Scalar(PrimitiveType::Usize),
        )
    } else if let Some(function) = owner.clone().and_then(ast::Fn::cast) {
        let function = graph.checked_function_header(&function)?;
        (function.body, function.params, function.result)
    } else if let Some(constant) = owner.and_then(ast::Const::cast) {
        let constant = graph.checked_constant_header(&constant)?;
        (constant.body, Vec::new(), constant.ty)
    } else {
        return Err(incomplete());
    };
    graph.result = graph.fresh()?;
    if let ConstType::Scalar(ty) = result {
        graph.known(graph.result, ty)?;
    } else {
        graph.opaque(graph.result)?;
    }
    graph.push()?;
    for (name, ty, _) in params {
        let id = graph.fresh()?;
        graph.known(id, ty)?;
        graph.bind(name, id)?;
    }
    if let Some(body) = graph.expr(body.0, 0)? {
        graph.combine(graph.result, body)?;
    }
    let selected = graph.selected.ok_or_else(incomplete)?;
    let selected = graph.root(selected)?;
    let node = graph.nodes[selected];
    // An unknown use cannot establish a default type. It also cannot erase
    // an independent concrete context (for example the actual owning return
    // type). The evaluator still checks every reached call/effect separately.
    // A boolean or floating cold use is not a numeric context for an integer
    // initializer. It must not turn the graph into a static type checker. The
    // real evaluator checks the initializer and each use only when entered.
    let concrete = node.concrete.filter(|ty| {
        node.family
            .is_none_or(|expected| family(*ty) == Some(expected))
    });
    if node.opaque && concrete.is_none() {
        return Err(incomplete());
    }
    let primitive = concrete
        .or(match node.family {
            Some(Family::Integer) => Some(PrimitiveType::I32),
            Some(Family::Float) => Some(PrimitiveType::F64),
            None => None,
        })
        .ok_or_else(incomplete)?;
    Ok(LocalNumericContext {
        primitive,
        work: graph.work,
        metadata: graph.bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        frontend::constants::{ConstLimits, evaluate},
        runtime::{shape::TypeId, value::ValueRepr},
    };
    const LOOP: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/stable-metric/ctfe-simple-loop.rs"
    );
    const CONTROL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/loop.rs"
    );
    const BASICS: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/basics.rs"
    );
    fn source(text: &str, edition: u16) -> ConstantSource {
        ConstantSource::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
            Digest([221; 32]),
            CfgSet::guest_default(),
        )
        .unwrap()
    }
    #[test]
    fn unchanged_ctfe_scalar_loop_infers_mutable_index_from_real_parameter_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            let source = source(LOOP, edition);
            let at = source
                .declarations()
                .iter()
                .find(|d| d.name() == "simple_loop")
                .unwrap()
                .range();
            for n in [0, 19, 35] {
                let plan = source
                    .const_call(
                        at,
                        vec![
                            ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), n).unwrap())
                                .unwrap(),
                        ],
                    )
                    .unwrap();
                let value = evaluate(&plan, ConstLimits::default()).unwrap();
                assert_eq!(
                    value.value().scalar().unwrap(),
                    ValueRepr::unsigned(TypeId(11), 0).unwrap()
                );
                assert_eq!(value.source(), Digest::of_bytes(LOOP.as_bytes()));
                assert_eq!(plan.profile().edition, edition);
            }
        }
    }
    #[test]
    fn unchanged_const_while_and_loop_components_preserve_unsuffixed_bindings() {
        for edition in [2015, 2018, 2021, 2024] {
            let source = source(CONTROL, edition);
            assert_eq!(
                source
                    .declarations()
                    .iter()
                    .filter(|d| d.is_anonymous())
                    .count(),
                5
            );
            assert!(source.named_constant("").is_err());
            assert!(source.named_constant("_").is_err());
            for marker in ["while x < 4", "if x == 4"] {
                let at = source
                    .declarations()
                    .iter()
                    .find(|d| {
                        CONTROL
                            .get(d.range().start as usize..d.range().end as usize)
                            .is_some_and(|text| text.contains(marker))
                    })
                    .unwrap()
                    .range();
                let plan = source.constant(at).unwrap();
                let result = evaluate(&plan, ConstLimits::default()).unwrap();
                assert_eq!(
                    result.value().scalar().unwrap(),
                    ValueRepr::signed(TypeId(6), 8).unwrap()
                );
                assert_eq!(result.source(), Digest::of_bytes(CONTROL.as_bytes()));
            }
        }
    }
    #[test]
    fn unchanged_gcd_returns_entered_branch_values_with_actual_parameter_types() {
        for edition in [2015, 2018, 2021, 2024] {
            let source = source(BASICS, edition);
            let at = source
                .declarations()
                .iter()
                .find(|d| d.name() == "gcd")
                .unwrap()
                .range();
            for (a, b) in [(48, 18), (18, 48)] {
                let arguments = [a, b]
                    .into_iter()
                    .map(|n| {
                        ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), n).unwrap())
                            .unwrap()
                    })
                    .collect();
                let plan = source.const_call(at, arguments).unwrap();
                let result = evaluate(&plan, ConstLimits::default()).unwrap();
                assert_eq!(
                    result.value().scalar().unwrap(),
                    ValueRepr::unsigned(TypeId(11), 6).unwrap()
                );
                assert_eq!(result.source(), Digest::of_bytes(BASICS.as_bytes()));
                assert_eq!(plan.profile().edition, edition);
            }
        }
    }
    #[test]
    fn unchanged_prime_loop_infers_divisor_from_actual_source_parameter() {
        for edition in [2015, 2018, 2021, 2024] {
            let source = source(BASICS, edition);
            let at = source
                .declarations()
                .iter()
                .find(|d| d.name() == "is_prime")
                .unwrap()
                .range();
            for (n, expected) in [(113, true), (117, false)] {
                let plan = source
                    .const_call(
                        at,
                        vec![
                            ConstValue::from_scalar(ValueRepr::unsigned(TypeId(12), n).unwrap())
                                .unwrap(),
                        ],
                    )
                    .unwrap();
                let result = evaluate(&plan, ConstLimits::default()).unwrap();
                assert_eq!(
                    result.value().scalar().unwrap(),
                    ValueRepr::boolean(expected)
                );
                assert_eq!(result.source(), Digest::of_bytes(BASICS.as_bytes()));
                assert_eq!(plan.profile().edition, edition);
            }
        }
    }
    fn result(
        text: &str,
    ) -> Result<crate::frontend::constants::ConstEvaluation, crate::frontend::constants::ConstFailure>
    {
        let source = source(text, 2024);
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == "A")
            .unwrap()
            .range();
        evaluate(&source.constant(at).unwrap(), ConstLimits::default())
    }
    #[test]
    fn source_scope_shadow_and_return_context_cannot_alias_other_numeric_bindings() {
        let value = result("const A:u8={let x=7; {let x=false; x;} x};").unwrap();
        assert_eq!(
            value.value().scalar().unwrap(),
            ValueRepr::unsigned(TypeId(9), 7).unwrap()
        );
        let value = result("const fn f()->u8{let x=9;return x;} const A:u8=f();").unwrap();
        assert_eq!(
            value.value().scalar().unwrap(),
            ValueRepr::unsigned(TypeId(9), 9).unwrap()
        );
    }
    #[test]
    fn connected_opaque_calls_do_not_mint_default_local_context() {
        let error = result("const A:i32={let x=0; missing(x); 1};").unwrap_err();
        assert_eq!(error.diagnostic().code, "LRR-E-CONST-UNSUPPORTED");
        assert!(error.panic().is_none());
    }
    #[test]
    fn cold_item_body_is_not_a_constraint_on_its_enclosing_local() {
        let value =
            result("const A:i32={let x=5; const fn cold()->bool{let x=\"wrong\";x} x};").unwrap();
        assert_eq!(
            value.value().scalar().unwrap(),
            ValueRepr::signed(TypeId(6), 5).unwrap()
        );
    }
    #[test]
    fn an_opaque_cold_use_cannot_erase_the_real_owning_return_context() {
        let value = result("const A:i32={let x=5;if false{missing(x);}x};").unwrap();
        assert_eq!(
            value.value().scalar().unwrap(),
            ValueRepr::signed(TypeId(6), 5).unwrap()
        );
        let value = result("const A:u8={let x=7;if false{missing(x);}x};").unwrap();
        assert_eq!(
            value.value().scalar().unwrap(),
            ValueRepr::unsigned(TypeId(9), 7).unwrap()
        );
        let error = result("const A:u8={let x=7;if true{missing(x);}x};").unwrap_err();
        assert_eq!(error.diagnostic().code, "LRR-E-CONST-SOURCE");
        assert!(error.panic().is_none());
    }
    #[test]
    fn cold_type_disagreements_do_not_precheck_the_selected_constant_body() {
        for edition in [2015, 2018, 2021, 2024] {
            for (text, expected) in [
                (
                    "const A:u8={let x=9;if false{let y:bool=x;}x};",
                    ValueRepr::unsigned(TypeId(9), 9).unwrap(),
                ),
                (
                    "const fn f()->u8{let x=7;return x;let y:bool=x;}const A:u8=f();",
                    ValueRepr::unsigned(TypeId(9), 7).unwrap(),
                ),
                (
                    "const A:i32={let x=9;if false{let y:bool=x;}1};",
                    ValueRepr::signed(TypeId(6), 1).unwrap(),
                ),
                (
                    "const A:i32={let x=9;if false{let y:f64=x;}1};",
                    ValueRepr::signed(TypeId(6), 1).unwrap(),
                ),
            ] {
                let source = source(text, edition);
                let at = source
                    .declarations()
                    .iter()
                    .find(|d| d.name() == "A")
                    .unwrap()
                    .range();
                let evaluation =
                    evaluate(&source.constant(at).unwrap(), ConstLimits::default()).unwrap();
                assert_eq!(evaluation.value().scalar().unwrap(), expected);
                assert_eq!(evaluation.source(), Digest::of_bytes(text.as_bytes()));
            }
        }
    }
    #[test]
    fn entered_type_disagreements_fail_at_the_actual_use_without_graph_diagnostics() {
        for edition in [2015, 2018, 2021, 2024] {
            let text = "const A:u8={let x=9;if true{let y:bool=x;}x};";
            let source = source(text, edition);
            let at = source
                .declarations()
                .iter()
                .find(|d| d.name() == "A")
                .unwrap()
                .range();
            let error =
                evaluate(&source.constant(at).unwrap(), ConstLimits::default()).unwrap_err();
            assert_eq!(error.diagnostic().code, "LRR-E-CONST-TYPE");
            let use_at = text.find("=x;").unwrap() + 1;
            assert_eq!(
                error.range(),
                SourceRange {
                    start: use_at as u32,
                    end: (use_at + 1) as u32
                }
            );
            assert!(error.panic().is_none());
        }
    }
    fn first_binding(source: &ConstantSource) -> StatementRef {
        let binding = source
            .0
            .parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::LetStmt::cast)
            .unwrap();
        StatementRef(ast::Stmt::LetStmt(binding))
    }
    #[test]
    fn header_and_namespace_probes_consume_the_callers_remaining_source_budget() {
        let source = source("const A:i32={let x=3;x};", 2024);
        let binding = first_binding(&source);
        assert_eq!(
            source
                .numeric_local_context(&binding, 1, 32 * 1024 * 1024)
                .unwrap_err()
                .code,
            "LRR-E-CONST-LIMIT"
        );
        assert_eq!(
            source
                .numeric_local_context(&binding, 100_000, 128)
                .unwrap_err()
                .code,
            "LRR-E-CONST-LIMIT"
        );
        let context = source
            .numeric_local_context(&binding, 100_000, 32 * 1024 * 1024)
            .unwrap();
        assert_eq!(context.primitive(), PrimitiveType::I32);
        assert!(context.work() > 1);
        assert!(context.metadata_bytes() > 128);
        let error = source
            .numeric_local_context(&binding, context.work() - 1, 32 * 1024 * 1024)
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-CONST-LIMIT");
    }
    #[test]
    fn a_cold_local_item_body_does_not_expand_the_source_constraint_work() {
        let before = source("const A:i32={let x=5;const fn cold()->bool{true}x};", 2024);
        let text = format!(
            "const A:i32={{let x=5;const fn cold()->bool{{{}true}}x}};",
            "unknown();".repeat(500)
        );
        let after = source(&text, 2024);
        let a = before
            .numeric_local_context(&first_binding(&before), 100_000, 32 * 1024 * 1024)
            .unwrap();
        let b = after
            .numeric_local_context(&first_binding(&after), 100_000, 32 * 1024 * 1024)
            .unwrap();
        assert_eq!(a.primitive(), b.primitive());
        assert_eq!(a.work(), b.work());
        assert_eq!(a.metadata_bytes(), b.metadata_bytes());
    }
}

#[cfg(test)]
#[path = "constant_length_context_tests.rs"]
mod length_context_tests;

#[cfg(test)]
mod family_priority_review_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        frontend::constants::{ConstLimits, evaluate},
        runtime::{shape::TypeId, value::ValueRepr},
    };
    fn evaluate_a(
        text: &str,
        edition: u16,
    ) -> Result<crate::frontend::constants::ConstEvaluation, crate::frontend::constants::ConstFailure>
    {
        let source = ConstantSource::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
            Digest([146; 32]),
            CfgSet::guest_default(),
        )
        .unwrap();
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == "A")
            .unwrap()
            .range();
        evaluate(&source.constant(at).unwrap(), ConstLimits::default())
    }
    #[test]
    fn incompatible_cold_context_cannot_suppress_a_later_actual_integer_constraint() {
        for edition in [2015, 2018, 2021, 2024] {
            for incompatible in ["bool", "f64"] {
                let text = format!(
                    "const A:i32={{let x=9;if false{{let bad:{incompatible}=x;}}let y:u8=x;y as i32}};"
                );
                let result = evaluate_a(&text, edition).unwrap();
                assert_eq!(
                    result.value().scalar().unwrap(),
                    ValueRepr::signed(TypeId(6), 9).unwrap()
                );
                assert_eq!(result.source(), Digest::of_bytes(text.as_bytes()));
            }
        }
    }
    #[test]
    fn entered_incompatible_context_still_fails_at_the_actual_variable_use() {
        for edition in [2015, 2018, 2021, 2024] {
            for incompatible in ["bool", "f64"] {
                let text = format!(
                    "const A:i32={{let x=9;if true{{let bad:{incompatible}=x;}}let y:u8=x;y as i32}};"
                );
                let failure = evaluate_a(&text, edition).unwrap_err();
                let at = text.find("=x;").unwrap() + 1;
                assert_eq!(failure.diagnostic().code, "LRR-E-CONST-TYPE");
                assert_eq!(
                    failure.range(),
                    SourceRange {
                        start: at as u32,
                        end: (at + 1) as u32
                    }
                );
                assert_eq!(failure.source(), Digest::of_bytes(text.as_bytes()));
                assert!(failure.panic().is_none());
            }
        }
    }
    #[test]
    fn compatible_owning_result_keeps_priority_over_later_compatible_cold_constraints() {
        for edition in [2015, 2018, 2021, 2024] {
            let text = "const A:u8={let x=9;if false{let bad:bool=x;let other:u16=x;}x};";
            let result = evaluate_a(text, edition).unwrap();
            assert_eq!(
                result.value().scalar().unwrap(),
                ValueRepr::unsigned(TypeId(9), 9).unwrap()
            );
        }
    }
}
