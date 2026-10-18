//! Pure original-body structure. Never resolves executable names or runs expansions.
use super::{
    ast_adapter::range,
    outline::BodyHandle,
    parse::ParsedFile,
    spans::{BodySpan, SourceRange},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    ir::pc::{NodeId, PcKey, Phase, SlotId, SlotKind},
    source::manifest::BodyKey,
};
use ra_ap_syntax::{
    AstNode, SyntaxNode, TextRange, TextSize,
    ast::{self, HasLoopBody},
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BodyIndexLimits {
    pub nodes: u32,
    pub slots: u32,
    pub metadata_bytes: usize,
}
impl Default for BodyIndexLimits {
    fn default() -> Self {
        Self {
            nodes: u32::MAX,
            slots: u32::MAX,
            metadata_bytes: usize::MAX,
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX", message.into())
}
fn limit(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX-LIMIT", message.into())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ExpressionKind {
    Block,
    If,
    Match,
    Loop,
    While,
    For,
    Break,
    Continue,
    Return,
    Call,
    MethodCall,
    Literal,
    Path,
    Closure,
    Async,
    Const,
    Gen,
    Macro,
    Other,
    Borrow,
    /// An expression rustc lowers to a call of a language-item function
    /// (`a..=b` is `RangeInclusive::new(a, b)`), lowered as an ordinary
    /// call of that function.
    LangCall,
}
impl ExpressionKind {
    fn tag(self) -> u32 {
        match self {
            Self::Block => 0,
            Self::If => 1,
            Self::Match => 2,
            Self::Loop => 3,
            Self::While => 4,
            Self::For => 5,
            Self::Break => 6,
            Self::Continue => 7,
            Self::Return => 8,
            Self::Call => 9,
            Self::MethodCall => 10,
            Self::Literal => 11,
            Self::Path => 12,
            Self::Closure => 13,
            Self::Async => 14,
            Self::Const => 15,
            Self::Gen => 16,
            Self::Macro => 17,
            Self::Other => 18,
            Self::Borrow => 19,
            Self::LangCall => 20,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NodeRole {
    Expression(ExpressionKind),
    Statement,
    Pattern,
    MatchArm,
    OpaqueItem,
    Syntax,
    /// Administrative return PC after the initializer's real syntax preorder.
    InitializerReturn,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ScopeId(pub u32);
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LabelId(pub u32);
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScopeKind {
    Block,
    Loop,
    MatchArm,
    Tail,
    Conditional,
    Clause,
    BooleanClause,
    Statement,
    MatchGuard,
    /// Exact non-block constant/static initializer, without constant evaluation.
    Initializer,
    /// A closure's parameters and body. Only the closure's own call frame
    /// enters it; the creating frame never does.
    Closure,
    /// The root scope of a function with pattern parameters (E-R5-010): the
    /// parameter bindings and their staged arguments live here, enclosing the
    /// body block's scope, so they drop after the body's locals.
    Parameters,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StructuralNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub role: NodeRole,
    pub range: SourceRange,
    pub scope: ScopeId,
    pub enclosing_syntax_target: Option<LabelId>,
    pub value: Option<SlotId>,
    pub place: Option<SlotId>,
    pub operands: Vec<NodeId>,
    /// Deterministic lazy value slots for runtime macro operands, scalar pattern
    /// tests, structural pattern projections and initialized reference bindings.
    /// These reserve frame metadata only; no argument syntax or cell is prepared.
    pub runtime_temporaries: [Option<SlotId>; 2],
    /// Bounded token-count reservation for reached intrinsic scalar operands.
    /// Indexing does not parse or resolve the token-tree expressions.
    pub runtime_scalar_slots: Vec<SlotId>,
    pub runtime_scalar_phases: u16,
    /// Exact syntactic unary minus over a (possibly grouped) integer token.
    /// No magnitude, suffix type, name or value is prepared while indexing.
    pub negative_integer_literal: bool,
    /// A constructor's ownership guard is separate from its result cell.
    /// Reserving this slot never binds a type or allocates guest storage.
    pub constructor_guard: Option<SlotId>,
    pub tail_scope: Option<ScopeId>,
    pub condition_scope: Option<ScopeId>,
    pub clause_scope: Option<ScopeId>,
    pub boolean_scope: Option<ScopeId>,
    pub statement_scope: Option<ScopeId>,
    pub guard_scope: Option<ScopeId>,
    /// A closure expression's own scope: its parameters, body and their
    /// temporaries belong to the closure's call frame, never to the frame
    /// that creates the closure value (which keeps the node's `scope`).
    pub closure_scope: Option<ScopeId>,
    pub extended_to: Option<super::temporary_extensions::ExtendedTo>,
    pub opaque: bool,
    /// A `let` statement whose pattern runs through the shared pattern
    /// transaction (destructuring, `@` bindings or `let`-`else`). Its first
    /// runtime temporary is the reserved Bool test result.
    pub pattern_statement: bool,
    /// A parenthesized expression (`( e )`, also the group a macro
    /// `$e:expr` fragment expands to): a transparent wrapper whose operand
    /// is the grouped expression.
    pub parenthesized: bool,
}
/// A closure parameter that is not a plain binding (`|&x|`, `|(a, b)|`,
/// `|ref x|`, `|x @ ..|`): the closure's parameters run the shared pattern
/// transaction against the call arguments.
pub(crate) fn closure_pattern_parameter(parameter: &ast::Param) -> bool {
    !matches!(parameter.pat(), Some(ast::Pat::IdentPat(pattern))
        if pattern.ref_token().is_none() && pattern.at_token().is_none())
}
pub(crate) fn closure_runs_patterns(closure: &ast::ClosureExpr) -> bool {
    closure
        .param_list()
        .is_some_and(|list| list.params().any(|parameter| closure_pattern_parameter(&parameter)))
}
/// `let` forms that need the shared pattern transaction. A plain identifier
/// or wildcard declaration binds directly; every other pattern is matched.
pub(crate) fn pattern_statement(statement: &ast::LetStmt) -> bool {
    if statement.let_else().is_some() {
        return true;
    }
    match statement.pat() {
        None | Some(ast::Pat::WildcardPat(_)) => false,
        Some(ast::Pat::IdentPat(binding)) => binding.pat().is_some(),
        Some(_) => true,
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LexicalScope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub node: NodeId,
    pub kind: ScopeKind,
    pub range: SourceRange,
}
/// Syntactic ancestry only; selecting a break/continue target is deferred.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LabelTarget {
    pub id: LabelId,
    pub parent: Option<LabelId>,
    pub node: NodeId,
    pub scope: ScopeId,
    pub is_loop: bool,
    pub spelling: Option<String>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BodyIndex {
    body: BodyKey,
    profile: ProfileIdentity,
    cfg: crate::source::snapshot::CfgSet,
    nodes: Vec<StructuralNode>,
    /// Derived from the explicitly encoded node ranges/IDs, never a second
    /// source identity. Equal ranges preserve original preorder selection.
    source_order: Vec<NodeId>,
    /// Derived from the encoded primary value slots after all nodes are final.
    /// Auxiliary slots and syntax-only nodes make slot/2 an invalid node ID.
    value_nodes: Vec<(SlotId, NodeId)>,
    pattern_bindings: super::pattern_bindings::PatternBindings,
    /// Derived once from the final immutable explicitly encoded structure.
    semantic_identity: crate::core::digest::Digest,
    scopes: Vec<LexicalScope>,
    labels: Vec<LabelTarget>,
    constructor_guards: Vec<(SlotId, NodeId)>,
    slots: u32,
    estimated_bytes: usize,
}
impl BodyIndex {
    pub(crate) fn semantic_identity(&self) -> crate::core::digest::Digest {
        self.semantic_identity
    }
    pub fn body(&self) -> &BodyKey {
        &self.body
    }
    pub fn profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub(crate) fn cfg(&self) -> &crate::source::snapshot::CfgSet {
        &self.cfg
    }
    pub fn nodes(&self) -> &[StructuralNode] {
        &self.nodes
    }
    pub(crate) fn pattern_bindings(&self) -> &super::pattern_bindings::PatternBindings {
        &self.pattern_bindings
    }
    /// Exact relative source location, retaining all role-distinct nodes at
    /// that location in original NodeId order. No source walk or resolution.
    pub(crate) fn nodes_at_range(
        &self,
        range: SourceRange,
    ) -> impl Iterator<Item = &StructuralNode> {
        let key = (range.start, range.end);
        let range_of = |id: &NodeId| {
            let range = self.nodes[id.0 as usize].range;
            (range.start, range.end)
        };
        let start = self.source_order.partition_point(|id| range_of(id) < key);
        let count = self.source_order[start..].partition_point(|id| range_of(id) == key);
        self.source_order[start..start + count]
            .iter()
            .map(|id| &self.nodes[id.0 as usize])
    }
    /// Syntactic pattern locations, not resolved binding declarations.
    pub fn pattern_locations(&self) -> impl Iterator<Item = &StructuralNode> {
        self.nodes
            .iter()
            .filter(|node| node.role == NodeRole::Pattern)
    }
    pub fn scopes(&self) -> &[LexicalScope] {
        &self.scopes
    }
    pub fn labels(&self) -> &[LabelTarget] {
        &self.labels
    }
    pub fn slot_count(&self) -> u32 {
        self.slots
    }
    pub(crate) fn value_node(&self, slot: SlotId) -> Option<&StructuralNode> {
        self.value_nodes
            .binary_search_by_key(&slot.0, |(value, _)| value.0)
            .ok()
            .and_then(|index| self.nodes.get(self.value_nodes[index].1.0 as usize))
    }
    pub fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }
    pub fn slot_kind(&self, id: SlotId) -> Result<SlotKind, Diagnostic> {
        if id.0 >= self.slots {
            return Err(fault("structural slot absent"));
        }
        Ok(
            if self
                .constructor_guards
                .binary_search_by_key(&id.0, |(slot, _)| slot.0)
                .is_ok()
            {
                SlotKind::Guard
            } else if id.0.is_multiple_of(2) {
                SlotKind::Value
            } else {
                SlotKind::Place
            },
        )
    }
    pub fn constructor_node(&self, guard: SlotId) -> Result<&StructuralNode, Diagnostic> {
        let ordinal = self
            .constructor_guards
            .binary_search_by_key(&guard.0, |(slot, _)| slot.0)
            .map_err(|_| fault("constructor guard absent"))?;
        self.nodes
            .get(self.constructor_guards[ordinal].1.0 as usize)
            .ok_or_else(|| fault("constructor guard node absent"))
    }
    pub fn span(&self, id: NodeId) -> Result<BodySpan, Diagnostic> {
        let node = self
            .nodes
            .get(id.0 as usize)
            .ok_or_else(|| fault("structural node absent"))?;
        Ok(BodySpan {
            body_key: self.body.clone(),
            relative_start: node.range.start,
            relative_end: node.range.end,
        })
    }
    /// The parameter list owning a pattern function's parameter patterns
    /// (E-R5-010), when this body is one.
    pub(crate) fn parameter_owner(&self) -> Option<NodeId> {
        self.scopes
            .first()
            .filter(|scope| scope.kind == ScopeKind::Parameters)
            .map(|scope| scope.node)
    }
    pub(crate) fn initializer_return_pc(&self) -> Result<Option<PcKey>, Diagnostic> {
        if self
            .scopes
            .first()
            .is_none_or(|scope| scope.kind != ScopeKind::Initializer)
        {
            return Ok(None);
        }
        let node = self
            .nodes
            .last()
            .filter(|node| node.role == NodeRole::InitializerReturn)
            .ok_or_else(|| fault("initializer return metadata absent"))?;
        self.pc(node.id, Phase::Entry).map(Some)
    }
    pub fn pc(&self, id: NodeId, phase: Phase) -> Result<PcKey, Diagnostic> {
        let node = self
            .nodes
            .get(id.0 as usize)
            .ok_or_else(|| fault("structural PC node absent"))?;
        if node.role == NodeRole::InitializerReturn && phase != Phase::Entry {
            return Err(fault("initializer return requires its entry phase"));
        }
        if let Phase::Operand(n) = phase
            && usize::from(n) >= node.operands.len()
            && !(node.role == NodeRole::Expression(ExpressionKind::Macro)
                && (n == 0 || n < node.runtime_scalar_phases)
                && node.runtime_temporaries[0].is_some())
        {
            return Err(fault("structural operand phase absent"));
        }
        Ok(PcKey {
            body: self.body.clone(),
            node: id.0,
            phase: phase.code()?,
        })
    }
}
impl StableEncode for BodyIndex {
    const DOMAIN: &'static str = "lrr/v5/original-body-structure";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.body);
        e.record(&self.profile);
        e.record(&self.cfg);
        e.record(&self.pattern_bindings);
        e.u32(self.slots);
        e.sequence(&self.nodes, |e, n| {
            e.u32(n.id.0);
            e.bool(n.parent.is_some());
            if let Some(p) = n.parent {
                e.u32(p.0);
            }
            match n.role {
                NodeRole::Expression(k) => {
                    e.u32(0);
                    e.u32(k.tag());
                }
                NodeRole::Statement => e.u32(1),
                NodeRole::Pattern => e.u32(2),
                NodeRole::MatchArm => e.u32(3),
                NodeRole::OpaqueItem => e.u32(4),
                NodeRole::Syntax => e.u32(5),
                NodeRole::InitializerReturn => e.u32(6),
            }
            e.u32(n.range.start);
            e.u32(n.range.end);
            e.u32(n.scope.0);
            e.bool(n.enclosing_syntax_target.is_some());
            if let Some(t) = n.enclosing_syntax_target {
                e.u32(t.0);
            }
            for s in [n.value, n.place] {
                e.bool(s.is_some());
                if let Some(s) = s {
                    e.u32(s.0);
                }
            }
            e.sequence(&n.operands, |e, n| e.u32(n.0));
            for slot in n.runtime_temporaries {
                e.option(&slot, |e, slot| e.u32(slot.0));
            }
            e.sequence(&n.runtime_scalar_slots, |e, slot| e.u32(slot.0));
            e.u32(u32::from(n.runtime_scalar_phases));
            e.bool(n.negative_integer_literal);
            e.option(&n.constructor_guard, |e, slot| e.u32(slot.0));
            e.option(&n.tail_scope, |e, scope| e.u32(scope.0));
            e.option(&n.condition_scope, |e, scope| e.u32(scope.0));
            e.option(&n.clause_scope, |e, scope| e.u32(scope.0));
            e.option(&n.boolean_scope, |e, scope| e.u32(scope.0));
            e.option(&n.statement_scope, |e, scope| e.u32(scope.0));
            e.option(&n.guard_scope, |e, scope| e.u32(scope.0));
            e.option(&n.closure_scope, |e, scope| e.u32(scope.0));
            e.option(&n.extended_to, |e, target| {
                e.u32(target.scope.0);
                e.sequence(&target.constructors, |e, node| e.u32(node.0));
            });
            e.bool(n.opaque);
            e.bool(n.pattern_statement);
            e.bool(n.parenthesized);
        });
        e.sequence(&self.scopes, |e, s| {
            e.u32(s.id.0);
            e.bool(s.parent.is_some());
            if let Some(p) = s.parent {
                e.u32(p.0);
            }
            e.u32(s.node.0);
            e.u32(match s.kind {
                ScopeKind::Block => 0,
                ScopeKind::Loop => 1,
                ScopeKind::MatchArm => 2,
                ScopeKind::Tail => 3,
                ScopeKind::Conditional => 4,
                ScopeKind::Clause => 5,
                ScopeKind::BooleanClause => 6,
                ScopeKind::Statement => 7,
                ScopeKind::MatchGuard => 8,
                ScopeKind::Initializer => 9,
                ScopeKind::Closure => 10,
                ScopeKind::Parameters => 11,
            });
            e.u32(s.range.start);
            e.u32(s.range.end);
        });
        e.sequence(&self.labels, |e, l| {
            e.u32(l.id.0);
            e.bool(l.parent.is_some());
            if let Some(p) = l.parent {
                e.u32(p.0);
            }
            e.u32(l.node.0);
            e.u32(l.scope.0);
            e.bool(l.is_loop);
            e.bool(l.spelling.is_some());
            if let Some(s) = &l.spelling {
                e.string(s);
            }
        });
    }
}
fn charge(bytes: &mut usize, amount: usize, budget: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= budget)
        .ok_or_else(|| limit("called body metadata budget"))?;
    Ok(())
}
fn negative_integer_literal(expr: &ast::Expr) -> bool {
    let ast::Expr::PrefixExpr(prefix) = expr else {
        return false;
    };
    if !prefix.op_token().is_some_and(|token| token.text() == "-") {
        return false;
    }
    let Some(mut operand) = prefix.expr() else {
        return false;
    };
    while let ast::Expr::ParenExpr(group) = operand {
        let Some(inner) = group.expr() else {
            return false;
        };
        operand = inner;
    }
    matches!(operand, ast::Expr::Literal(literal)
        if literal.token().kind() == ra_ap_syntax::SyntaxKind::INT_NUMBER)
}

fn expression_kind(expr: &ast::Expr) -> ExpressionKind {
    match expr {
        ast::Expr::BlockExpr(b) => {
            if b.async_token().is_some() {
                ExpressionKind::Async
            } else if b.const_token().is_some() {
                ExpressionKind::Const
            } else if b.gen_token().is_some() {
                ExpressionKind::Gen
            } else {
                ExpressionKind::Block
            }
        }
        ast::Expr::IfExpr(_) => ExpressionKind::If,
        ast::Expr::MatchExpr(_) => ExpressionKind::Match,
        ast::Expr::LoopExpr(_) => ExpressionKind::Loop,
        ast::Expr::WhileExpr(_) => ExpressionKind::While,
        ast::Expr::ForExpr(_) => ExpressionKind::For,
        ast::Expr::BreakExpr(_) => ExpressionKind::Break,
        ast::Expr::ContinueExpr(_) => ExpressionKind::Continue,
        ast::Expr::ReturnExpr(_) => ExpressionKind::Return,
        ast::Expr::CallExpr(_) => ExpressionKind::Call,
        ast::Expr::MethodCallExpr(_) => ExpressionKind::MethodCall,
        ast::Expr::Literal(_) => ExpressionKind::Literal,
        ast::Expr::RefExpr(_) => ExpressionKind::Borrow,
        ast::Expr::PathExpr(_) => ExpressionKind::Path,
        ast::Expr::ClosureExpr(_) => ExpressionKind::Closure,
        ast::Expr::MacroExpr(_) => ExpressionKind::Macro,
        ast::Expr::RangeExpr(range) if super::constructors::range_inclusive_bounds(range).is_some() => {
            ExpressionKind::LangCall
        }
        _ => ExpressionKind::Other,
    }
}
/// The declaration a non-block body root is the initializer of: a `const` or
/// `static` item, or the `= expr` of an enum variant, an anonymous constant of
/// the enum's representation type (E-R5-013).
pub(crate) fn is_initializer_owner(owner: &SyntaxNode) -> bool {
    ast::Const::can_cast(owner.kind())
        || ast::Static::can_cast(owner.kind())
        || (ast::ConstArg::can_cast(owner.kind())
            && owner
                .parent()
                .is_some_and(|variant| ast::Variant::can_cast(variant.kind())))
}
impl ParsedFile {
    /// Reuse the exact pinned AST body boundary for function blocks and captured
    /// constant/static initializer expressions. This does not evaluate constants.
    pub(crate) fn entered_body_root(&self, body: &BodyHandle) -> Result<SyntaxNode, Diagnostic> {
        body.require_entry()?;
        let tree = self.parsed.tree();
        let full = range(tree.syntax().text_range());
        if body.range.start > body.range.end || body.range.end > full.end {
            return Err(fault("body source range outside parse"));
        }
        let covered = tree.syntax().covering_element(TextRange::new(
            TextSize::from(body.range.start),
            TextSize::from(body.range.end),
        ));
        // A single-token initializer is covered by its token. The pinned
        // SyntaxElement ancestry includes its actual LiteralExpr boundary.
        let root = covered
            .ancestors()
            .find(|node| range(node.text_range()) == body.range && ast::Expr::can_cast(node.kind()))
            // A function with a pattern parameter spans its parameter list
            // through its block (E-R5-010): the root is the block.
            .or_else(|| super::fn_parameters::pattern_function_block(&covered, body.range))
            .ok_or_else(|| fault("body expression syntax boundary absent"))?;
        if super::fn_parameters::body_range(&root) != body.range
            || !ast::Expr::can_cast(root.kind())
            || self.fn_body_identity(&root)? != body.body_hash
        {
            return Err(fault("body source range/content mismatch"));
        }
        if !ast::BlockExpr::can_cast(root.kind())
            && !root.parent().is_some_and(|parent| is_initializer_owner(&parent))
        {
            return Err(fault("non-block body is not a constant/static initializer"));
        }
        Ok(root)
    }
    pub fn index_body(
        &self,
        body: &BodyHandle,
        limits: BodyIndexLimits,
    ) -> Result<BodyIndex, Diagnostic> {
        self.index_body_with_cfg(
            body,
            &crate::source::snapshot::CfgSet::guest_default(),
            limits,
        )
    }
    pub(crate) fn index_body_with_cfg(
        &self,
        body: &BodyHandle,
        cfg: &crate::source::snapshot::CfgSet,
        limits: BodyIndexLimits,
    ) -> Result<BodyIndex, Diagnostic> {
        body.require_entry()?;
        let root = self.entered_body_root(body)?;
        let cfg_bytes = cfg.metadata_bytes()?;
        if cfg_bytes > limits.metadata_bytes {
            return Err(limit("body cfg metadata budget"));
        }
        let mut index = BodyIndex {
            body: body.key(),
            profile: self.profile.clone(),
            cfg: cfg.clone(),
            nodes: Vec::new(),
            source_order: Vec::new(),
            value_nodes: Vec::new(),
            pattern_bindings: super::pattern_bindings::PatternBindings::empty(),
            semantic_identity: crate::core::digest::Digest([0; 32]),
            scopes: Vec::new(),
            labels: Vec::new(),
            constructor_guards: Vec::new(),
            slots: 0,
            estimated_bytes: cfg_bytes,
        };
        charge(
            &mut index.estimated_bytes,
            std::mem::size_of::<Vec<NodeId>>(),
            limits.metadata_bytes,
        )?;
        charge(
            &mut index.estimated_bytes,
            std::mem::size_of::<crate::core::digest::Digest>(),
            limits.metadata_bytes,
        )?;
        // Non-block initializers need an enclosing lifetime boundary even
        // when their root is itself a loop with a separate control scope.
        let initializer_scope = if ast::BlockExpr::can_cast(root.kind()) {
            None
        } else {
            charge(&mut index.estimated_bytes, 64, limits.metadata_bytes)?;
            index
                .scopes
                .try_reserve_exact(1)
                .map_err(|_| limit("initializer scope allocation"))?;
            let id = ScopeId(0);
            index.scopes.push(LexicalScope {
                id,
                parent: None,
                node: NodeId(0),
                kind: ScopeKind::Initializer,
                range: SourceRange {
                    start: 0,
                    end: body
                        .range
                        .end
                        .checked_sub(body.range.start)
                        .ok_or_else(|| fault("initializer range is reversed"))?,
                },
            });
            Some(id)
        };
        struct Frame {
            syntax: SyntaxNode,
            parent: Option<NodeId>,
            expression: Option<NodeId>,
            scope: Option<ScopeId>,
            target: Option<LabelId>,
        }
        // A function with pattern parameters (E-R5-010): the parameter list is
        // indexed after the block, in a root scope that encloses the block's.
        let parameter_list = super::fn_parameters::pattern_list(&root);
        let parameter_scope = if parameter_list.is_some() {
            charge(&mut index.estimated_bytes, 64, limits.metadata_bytes)?;
            index
                .scopes
                .try_reserve_exact(1)
                .map_err(|_| limit("parameter scope allocation"))?;
            let id = ScopeId(0);
            index.scopes.push(LexicalScope {
                id,
                parent: None,
                node: NodeId(0),
                kind: ScopeKind::Parameters,
                range: SourceRange {
                    start: 0,
                    end: body
                        .range
                        .end
                        .checked_sub(body.range.start)
                        .ok_or_else(|| fault("parameter body range is reversed"))?,
                },
            });
            Some(id)
        } else {
            None
        };
        let mut stack = Vec::new();
        if let Some(list) = &parameter_list {
            stack.push(Frame {
                syntax: list.syntax().clone(),
                parent: None,
                expression: None,
                scope: parameter_scope,
                target: None,
            });
        }
        stack.push(Frame {
            syntax: root.clone(),
            parent: None,
            expression: None,
            scope: initializer_scope.or(parameter_scope),
            target: None,
        });
        let mut scope_roots = std::collections::BTreeMap::new();
        let mut clause_roots: std::collections::BTreeMap<(u32, u32), ScopeId> =
            std::collections::BTreeMap::new();
        let mut boolean_roots: std::collections::BTreeMap<(u32, u32), ScopeId> =
            std::collections::BTreeMap::new();
        let mut chain_aggregators = std::collections::BTreeSet::new();
        while let Some(mut frame) = stack.pop() {
            if ast::Expr::can_cast(frame.syntax.kind()) {
                let r = range(frame.syntax.text_range());
                if let Some(tail) = scope_roots.get(&(r.start, r.end)) {
                    frame.scope = Some(*tail);
                }
            }
            if index.nodes.len() >= limits.nodes as usize {
                return Err(limit("called body node budget"));
            }
            charge(
                &mut index.estimated_bytes,
                std::mem::size_of::<StructuralNode>().max(192),
                limits.metadata_bytes,
            )?;
            let id = NodeId(
                u32::try_from(index.nodes.len())
                    .map_err(|_| limit("body node ordinal overflow"))?,
            );
            let r = range(frame.syntax.text_range());
            let clause_scope = clause_roots.get(&(r.start, r.end)).copied();
            if let Some(clause) = clause_scope {
                index.scopes[clause.0 as usize].node = id;
            }
            let boolean_scope = boolean_roots.get(&(r.start, r.end)).copied();
            if let Some(boolean) = boolean_scope {
                index.scopes[boolean.0 as usize].node = id;
            }
            let expr = ast::Expr::cast(frame.syntax.clone());
            let kind = expr.as_ref().map(expression_kind);
            // The pattern function's parameter list: the owner of the shared
            // pattern transaction for its parameter patterns.
            let parameter_owner = frame.parent.is_none()
                && parameter_list
                    .as_ref()
                    .is_some_and(|list| list.syntax() == &frame.syntax);
            if parameter_owner && let Some(parameters) = parameter_scope {
                index.scopes[parameters.0 as usize].node = id;
            }
            let item = ast::Item::can_cast(frame.syntax.kind());
            let role = if item {
                NodeRole::OpaqueItem
            } else if let Some(k) = kind {
                NodeRole::Expression(k)
            } else if ast::Stmt::can_cast(frame.syntax.kind()) {
                NodeRole::Statement
            } else if ast::Pat::can_cast(frame.syntax.kind()) {
                NodeRole::Pattern
            } else if ast::MatchArm::can_cast(frame.syntax.kind()) {
                NodeRole::MatchArm
            } else {
                NodeRole::Syntax
            };
            let opaque = item
                || matches!(
                    kind,
                    Some(
                        ExpressionKind::Async
                            | ExpressionKind::Const
                            | ExpressionKind::Gen
                            | ExpressionKind::Macro
                    )
                );
            let r = range(frame.syntax.text_range());
            let relative = SourceRange {
                start: r
                    .start
                    .checked_sub(body.range.start)
                    .ok_or_else(|| fault("node outside body"))?,
                end: r
                    .end
                    .checked_sub(body.range.start)
                    .ok_or_else(|| fault("node outside body"))?,
            };
            let is_loop = matches!(
                kind,
                Some(ExpressionKind::Loop | ExpressionKind::While | ExpressionKind::For)
            );
            let scope_kind = if is_loop {
                Some(ScopeKind::Loop)
            } else if kind == Some(ExpressionKind::Block) {
                Some(ScopeKind::Block)
            } else if role == NodeRole::MatchArm {
                Some(ScopeKind::MatchArm)
            } else {
                None
            };
            let scope = if let Some(k) = scope_kind {
                charge(&mut index.estimated_bytes, 64, limits.metadata_bytes)?;
                let scope = ScopeId(
                    u32::try_from(index.scopes.len())
                        .map_err(|_| limit("body scope ordinal overflow"))?,
                );
                index
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| limit("body scope allocation"))?;
                index.scopes.push(LexicalScope {
                    id: scope,
                    parent: frame.scope,
                    node: id,
                    kind: k,
                    range: relative,
                });
                scope
            } else {
                frame.scope.ok_or_else(|| fault("body root scope absent"))?
            };
            let statement_scope = if ast::LetStmt::can_cast(frame.syntax.kind())
                || ast::ExprStmt::can_cast(frame.syntax.kind())
            {
                charge(&mut index.estimated_bytes, 64, limits.metadata_bytes)?;
                let id_scope = ScopeId(
                    u32::try_from(index.scopes.len())
                        .map_err(|_| limit("statement scope ordinal overflow"))?,
                );
                index
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| limit("statement scope allocation"))?;
                index.scopes.push(LexicalScope {
                    id: id_scope,
                    parent: Some(scope),
                    node: id,
                    kind: ScopeKind::Statement,
                    range: relative,
                });
                Some(id_scope)
            } else {
                None
            };
            let tail_scope = if let Some(ast::Expr::BlockExpr(block)) = &expr
                && kind == Some(ExpressionKind::Block)
                && !opaque
            {
                if let Some(tail) = block.tail_expr() {
                    charge(&mut index.estimated_bytes, 128, limits.metadata_bytes)?;
                    let r = range(tail.syntax().text_range());
                    let relative = SourceRange {
                        start: r
                            .start
                            .checked_sub(body.range.start)
                            .ok_or_else(|| fault("tail precedes body"))?,
                        end: r
                            .end
                            .checked_sub(body.range.start)
                            .ok_or_else(|| fault("tail precedes body"))?,
                    };
                    let tail_scope = ScopeId(
                        u32::try_from(index.scopes.len())
                            .map_err(|_| limit("tail scope ordinal overflow"))?,
                    );
                    index
                        .scopes
                        .try_reserve(1)
                        .map_err(|_| limit("tail scope allocation"))?;
                    index.scopes.push(LexicalScope {
                        id: tail_scope,
                        parent: Some(scope),
                        node: id,
                        kind: ScopeKind::Tail,
                        range: relative,
                    });
                    if scope_roots.insert((r.start, r.end), tail_scope).is_some() {
                        return Err(fault("duplicate tail root"));
                    }
                    Some(tail_scope)
                } else {
                    None
                }
            } else {
                None
            };
            // Only structural ranges are indexed here; no condition operand is prepared.
            let condition_parts = match &expr {
                Some(ast::Expr::IfExpr(x)) => x
                    .condition()
                    .map(|condition| (condition, x.then_branch().map(ast::Expr::BlockExpr))),
                Some(ast::Expr::WhileExpr(x)) => x
                    .condition()
                    .map(|condition| (condition, x.loop_body().map(ast::Expr::BlockExpr))),
                _ => None,
            };
            // `for PAT in HEAD BODY` (rustc's lowering: `match
            // IntoIterator::into_iter(HEAD) { mut iter => loop { match
            // Iterator::next(&mut iter) { None => break, Some(PAT) => BODY } } }`):
            // the node's own Loop scope owns the iterator for the whole loop;
            // this child scope is one iteration (the `next` result, PAT's
            // bindings and BODY) and is the target of `break`/`continue`.
            let for_iteration = if let Some(ast::Expr::ForExpr(for_expr)) = &expr {
                charge(&mut index.estimated_bytes, 128, limits.metadata_bytes)?;
                let iteration = ScopeId(
                    u32::try_from(index.scopes.len())
                        .map_err(|_| limit("for iteration scope ordinal overflow"))?,
                );
                index
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| limit("for iteration scope allocation"))?;
                index.scopes.push(LexicalScope {
                    id: iteration,
                    parent: Some(scope),
                    node: id,
                    kind: ScopeKind::Loop,
                    range: relative,
                });
                if let Some(body) = for_expr.loop_body() {
                    let r = range(body.syntax().text_range());
                    if scope_roots.insert((r.start, r.end), iteration).is_some() {
                        return Err(fault("duplicate for body root"));
                    }
                }
                Some(iteration)
            } else {
                None
            };
            let condition_scope = if let Some((condition, consequent)) = condition_parts {
                charge(&mut index.estimated_bytes, 192, limits.metadata_bytes)?;
                let r = range(condition.syntax().text_range());
                let condition_scope = ScopeId(
                    u32::try_from(index.scopes.len())
                        .map_err(|_| limit("condition scope ordinal overflow"))?,
                );
                index
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| limit("condition scope allocation"))?;
                index.scopes.push(LexicalScope {
                    id: condition_scope,
                    parent: Some(scope),
                    node: id,
                    kind: ScopeKind::Conditional,
                    range: relative,
                });
                if scope_roots
                    .insert((r.start, r.end), condition_scope)
                    .is_some()
                {
                    return Err(fault("duplicate condition root"));
                }
                // Index written top-level clauses only. Grammar validity remains a
                // reached ConditionPlan check, including misplaced/cold let syntax.
                let mut pending = vec![condition];
                let mut retained = condition_scope;
                while let Some(clause) = pending.pop() {
                    charge(&mut index.estimated_bytes, 128, limits.metadata_bytes)?;
                    if let ast::Expr::BinExpr(binary) = &clause
                        && binary.op_token().is_some_and(|t| t.text() == "&&")
                        && let (Some(left), Some(right)) = (binary.lhs(), binary.rhs())
                    {
                        let r = range(clause.syntax().text_range());
                        chain_aggregators.insert((r.start, r.end));
                        pending
                            .try_reserve(2)
                            .map_err(|_| limit("clause traversal allocation"))?;
                        pending.push(right);
                        pending.push(left);
                        continue;
                    }
                    let r = range(clause.syntax().text_range());
                    if matches!(clause, ast::Expr::LetExpr(_)) {
                        charge(&mut index.estimated_bytes, 128, limits.metadata_bytes)?;
                        let scope = ScopeId(
                            u32::try_from(index.scopes.len())
                                .map_err(|_| limit("clause scope ordinal overflow"))?,
                        );
                        index
                            .scopes
                            .try_reserve(1)
                            .map_err(|_| limit("clause scope allocation"))?;
                        index.scopes.push(LexicalScope {
                            id: scope,
                            parent: Some(retained),
                            node: id,
                            kind: ScopeKind::Clause,
                            range: SourceRange {
                                start: r
                                    .start
                                    .checked_sub(body.range.start)
                                    .ok_or_else(|| fault("clause precedes body"))?,
                                end: r
                                    .end
                                    .checked_sub(body.range.start)
                                    .ok_or_else(|| fault("clause precedes body"))?,
                            },
                        });
                        if clause_roots.insert((r.start, r.end), scope).is_some() {
                            return Err(fault("duplicate clause root"));
                        }
                        retained = scope;
                    } else {
                        charge(&mut index.estimated_bytes, 128, limits.metadata_bytes)?;
                        let scope = ScopeId(
                            u32::try_from(index.scopes.len())
                                .map_err(|_| limit("boolean clause scope ordinal overflow"))?,
                        );
                        index
                            .scopes
                            .try_reserve(1)
                            .map_err(|_| limit("boolean clause scope allocation"))?;
                        index.scopes.push(LexicalScope {
                            id: scope,
                            parent: Some(retained),
                            node: id,
                            kind: ScopeKind::BooleanClause,
                            range: SourceRange {
                                start: r
                                    .start
                                    .checked_sub(body.range.start)
                                    .ok_or_else(|| fault("boolean clause precedes body"))?,
                                end: r
                                    .end
                                    .checked_sub(body.range.start)
                                    .ok_or_else(|| fault("boolean clause precedes body"))?,
                            },
                        });
                        if boolean_roots.insert((r.start, r.end), scope).is_some() {
                            return Err(fault("duplicate boolean clause root"));
                        }
                        scope_roots.insert((r.start, r.end), scope);
                        continue;
                    }
                    scope_roots.insert((r.start, r.end), retained);
                }
                if let Some(consequent) = consequent {
                    let r = range(consequent.syntax().text_range());
                    if scope_roots.insert((r.start, r.end), retained).is_some() {
                        return Err(fault("duplicate consequent root"));
                    }
                }
                Some(condition_scope)
            } else {
                for_iteration
            };
            if let Some(ast::Expr::BinExpr(binary)) = &expr
                && binary
                    .op_token()
                    .is_some_and(|t| matches!(t.text(), "&&" | "||"))
                && !chain_aggregators.contains(&(r.start, r.end))
            {
                for operand in [binary.lhs(), binary.rhs()].into_iter().flatten() {
                    let r = range(operand.syntax().text_range());
                    if boolean_roots.contains_key(&(r.start, r.end))
                        || clause_roots.contains_key(&(r.start, r.end))
                    {
                        continue;
                    }
                    charge(&mut index.estimated_bytes, 192, limits.metadata_bytes)?;
                    let boolean = ScopeId(
                        u32::try_from(index.scopes.len())
                            .map_err(|_| limit("lazy operand scope ordinal overflow"))?,
                    );
                    index
                        .scopes
                        .try_reserve(1)
                        .map_err(|_| limit("lazy operand scope allocation"))?;
                    index.scopes.push(LexicalScope {
                        id: boolean,
                        parent: Some(scope),
                        node: id,
                        kind: ScopeKind::BooleanClause,
                        range: SourceRange {
                            start: r
                                .start
                                .checked_sub(body.range.start)
                                .ok_or_else(|| fault("lazy operand precedes body"))?,
                            end: r
                                .end
                                .checked_sub(body.range.start)
                                .ok_or_else(|| fault("lazy operand precedes body"))?,
                        },
                    });
                    if scope_roots.insert((r.start, r.end), boolean).is_some() {
                        return Err(fault("conflicting lazy operand scope root"));
                    }
                    boolean_roots.insert((r.start, r.end), boolean);
                }
            }
            let guard_scope = if let Some(arm) = ast::MatchArm::cast(frame.syntax.clone())
                && let Some(guard) = arm.guard().and_then(|guard| guard.condition())
            {
                charge(&mut index.estimated_bytes, 192, limits.metadata_bytes)?;
                let r = range(guard.syntax().text_range());
                let guard_scope = ScopeId(
                    u32::try_from(index.scopes.len())
                        .map_err(|_| limit("match guard scope ordinal overflow"))?,
                );
                index
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| limit("match guard scope allocation"))?;
                index.scopes.push(LexicalScope {
                    id: guard_scope,
                    parent: Some(scope),
                    node: id,
                    kind: ScopeKind::MatchGuard,
                    range: SourceRange {
                        start: r
                            .start
                            .checked_sub(body.range.start)
                            .ok_or_else(|| fault("guard precedes body"))?,
                        end: r
                            .end
                            .checked_sub(body.range.start)
                            .ok_or_else(|| fault("guard precedes body"))?,
                    },
                });
                if scope_roots.insert((r.start, r.end), guard_scope).is_some()
                    || boolean_roots
                        .insert((r.start, r.end), guard_scope)
                        .is_some()
                {
                    return Err(fault("conflicting match guard root"));
                }
                Some(guard_scope)
            } else {
                None
            };
            let closure_scope = if kind == Some(ExpressionKind::Closure) {
                charge(&mut index.estimated_bytes, 64, limits.metadata_bytes)?;
                let closure_scope = ScopeId(
                    u32::try_from(index.scopes.len())
                        .map_err(|_| limit("closure scope ordinal overflow"))?,
                );
                index
                    .scopes
                    .try_reserve(1)
                    .map_err(|_| limit("closure scope allocation"))?;
                index.scopes.push(LexicalScope {
                    id: closure_scope,
                    parent: Some(scope),
                    node: id,
                    kind: ScopeKind::Closure,
                    range: relative,
                });
                Some(closure_scope)
            } else {
                None
            };
            let label = match &expr {
                Some(ast::Expr::LoopExpr(x)) => x.label(),
                Some(ast::Expr::WhileExpr(x)) => x.label(),
                Some(ast::Expr::ForExpr(x)) => x.label(),
                Some(ast::Expr::BlockExpr(x)) if !opaque => x.label(),
                _ => None,
            };
            let target = if is_loop || label.is_some() {
                let spelling = label
                    .and_then(|l| l.lifetime())
                    .and_then(|l| l.lifetime_ident_token())
                    .map(|t| t.text().to_string());
                charge(
                    &mut index.estimated_bytes,
                    64usize
                        .checked_add(spelling.as_ref().map_or(0, String::len))
                        .ok_or_else(|| limit("label metadata overflow"))?,
                    limits.metadata_bytes,
                )?;
                let target = LabelId(
                    u32::try_from(index.labels.len())
                        .map_err(|_| limit("label ordinal overflow"))?,
                );
                index
                    .labels
                    .try_reserve(1)
                    .map_err(|_| limit("label allocation"))?;
                index.labels.push(LabelTarget {
                    id: target,
                    parent: frame.target,
                    node: id,
                    // `break`/`continue` of a `for` leave one iteration; the
                    // loop's own scope (its iterator) is left by its exit.
                    scope: for_iteration.unwrap_or(scope),
                    is_loop,
                    spelling,
                });
                Some(target)
            } else {
                frame.target
            };
            let (value, place) = if kind.is_some() {
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("structural slot budget"))?;
                let pair = (Some(SlotId(index.slots)), Some(SlotId(index.slots + 1)));
                index.slots = next;
                if let Some(parent) = frame.expression {
                    let operands = &mut index.nodes[parent.0 as usize].operands;
                    if operands.len() >= usize::from(u16::MAX - 3) {
                        return Err(limit("operand phase budget"));
                    }
                    charge(&mut index.estimated_bytes, 4, limits.metadata_bytes)?;
                    operands
                        .try_reserve(1)
                        .map_err(|_| limit("operand allocation"))?;
                    operands.push(id);
                }
                pair
            } else {
                (None, None)
            };
            let runtime_temporaries = if kind == Some(ExpressionKind::For) {
                // The iterator (loop-owned) and each iteration's `next` result.
                let next = index
                    .slots
                    .checked_add(4)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("for temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), Some(SlotId(index.slots + 2))];
                index.slots = next;
                slots
            } else if kind == Some(ExpressionKind::Match) {
                let next = index
                    .slots
                    .checked_add(4)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("scalar match temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), Some(SlotId(index.slots + 2))];
                index.slots = next;
                slots
            } else if kind == Some(ExpressionKind::Macro) {
                let next = index
                    .slots
                    .checked_add(4)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("runtime macro temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), Some(SlotId(index.slots + 2))];
                index.slots = next;
                slots
            } else if ast::BinExpr::cast(frame.syntax.clone()).is_some_and(|binary| {
                binary.op_token().is_some_and(|token| {
                    matches!(
                        token.text(),
                        "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
                    )
                })
            }) {
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|slots| *slots <= limits.slots)
                    .ok_or_else(|| limit("compound assignment temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), None];
                index.slots = next;
                slots
            } else if frame.syntax.kind() == ra_ap_syntax::SyntaxKind::LET_STMT
                && ast::LetStmt::cast(frame.syntax.clone()).is_some_and(|statement| {
                    statement.initializer().is_some()
                        && statement.let_else().is_none()
                        && matches!(statement.pat(), Some(ast::Pat::IdentPat(pattern))
                            if pattern.ref_token().is_some() && pattern.at_token().is_none())
                })
            {
                // Reserve the reference result separately from its referent.
                // Preserve even value/odd place slot identities and bounds.
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("reference binding temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), None];
                index.slots = next;
                slots
            } else if kind == Some(ExpressionKind::MethodCall) {
                // The adjusted receiver argument (autoref/autoderef result),
                // distinct from the receiver expression's own value/place.
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("method receiver slot budget"))?;
                // Receiver argument value slot; its reserved place pair is the
                // autoderef scratch place.
                let slots = [Some(SlotId(index.slots)), None];
                index.slots = next;
                slots
            } else if frame.syntax.kind() == ra_ap_syntax::SyntaxKind::LET_STMT
                && ast::LetStmt::cast(frame.syntax.clone())
                    .is_some_and(|statement| pattern_statement(&statement))
            {
                // The shared pattern transaction's Bool test result, owned by
                // the statement scope like the initializer's temporaries.
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("pattern statement temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), None];
                index.slots = next;
                slots
            } else if parameter_owner {
                // The shared pattern transaction's Bool test result for the
                // function's pattern parameters, owned by its parameter scope.
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("parameter pattern temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), None];
                index.slots = next;
                slots
            } else if ast::ClosureExpr::cast(frame.syntax.clone())
                .is_some_and(|closure| closure_runs_patterns(&closure))
            {
                // The shared pattern transaction's Bool test result for the
                // closure's pattern parameters, owned by the closure frame.
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("closure pattern temporary slot budget"))?;
                let slots = [Some(SlotId(index.slots)), None];
                index.slots = next;
                slots
            } else {
                [None, None]
            };
            let (runtime_scalar_slots, runtime_scalar_phases) =
                if kind == Some(ExpressionKind::Macro) {
                    // Tokens provide an upper bound on scalar expression storage;
                    // leave the opaque argument grammar and all message tails cold.
                    let tokens = frame
                        .syntax
                        .descendants_with_tokens()
                        .filter_map(|element| element.into_token())
                        .filter(|token| !token.kind().is_trivia())
                        .count();
                    let count =
                        u32::try_from(tokens).map_err(|_| limit("macro token count overflow"))?;
                    let phases = count
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(4))
                        .filter(|n| *n <= u32::from(u16::MAX - 3))
                        .ok_or_else(|| limit("macro scalar phase budget"))?;
                    let next = count
                        .checked_mul(2)
                        .and_then(|n| index.slots.checked_add(n))
                        .filter(|n| *n <= limits.slots)
                        .ok_or_else(|| limit("macro scalar slot budget"))?;
                    charge(
                        &mut index.estimated_bytes,
                        tokens
                            .checked_mul(std::mem::size_of::<SlotId>())
                            .ok_or_else(|| limit("macro scalar metadata overflow"))?,
                        limits.metadata_bytes,
                    )?;
                    let mut slots = Vec::new();
                    slots
                        .try_reserve_exact(tokens)
                        .map_err(|_| limit("macro scalar reservation"))?;
                    for offset in 0..count {
                        slots.push(SlotId(index.slots + offset * 2));
                    }
                    index.slots = next;
                    (slots, phases as u16)
                } else if parameter_owner {
                    // One staging value slot (with its place pair) per written
                    // parameter, owned by the function's parameter scope.
                    let count = u32::try_from(
                        parameter_list.as_ref().map_or(0, |list| list.params().count()),
                    )
                    .map_err(|_| limit("function parameter count overflow"))?;
                    let next = count
                        .checked_mul(2)
                        .and_then(|n| index.slots.checked_add(n))
                        .filter(|n| *n <= limits.slots)
                        .ok_or_else(|| limit("function parameter slot budget"))?;
                    let slots = (0..count).map(|offset| SlotId(index.slots + offset * 2)).collect();
                    index.slots = next;
                    (slots, 0)
                } else if let Some(closure) = ast::ClosureExpr::cast(frame.syntax.clone()) {
                    // One parameter staging value slot (with its place pair)
                    // per closure parameter, owned by the closure frame.
                    let count = u32::try_from(
                        closure.param_list().map_or(0, |list| list.params().count()),
                    )
                    .map_err(|_| limit("closure parameter count overflow"))?;
                    let next = count
                        .checked_mul(2)
                        .and_then(|n| index.slots.checked_add(n))
                        .filter(|n| *n <= limits.slots)
                        .ok_or_else(|| limit("closure parameter slot budget"))?;
                    let slots = (0..count).map(|offset| SlotId(index.slots + offset * 2)).collect();
                    index.slots = next;
                    (slots, 0)
                } else {
                    (Vec::new(), 0)
                };
            let constructor_guard = if expr.as_ref().is_some_and(|expr| {
                matches!(
                    expr,
                    ast::Expr::TupleExpr(_) | ast::Expr::ArrayExpr(_) | ast::Expr::RecordExpr(_)
                ) || matches!(expr, ast::Expr::RangeExpr(range)
                    if super::constructors::LangRecord::of(range).is_some())
            }) {
                // Keep the established value/place parity for every other slot.
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("constructor guard slot budget"))?;
                charge(
                    &mut index.estimated_bytes,
                    std::mem::size_of::<(SlotId, NodeId)>(),
                    limits.metadata_bytes,
                )?;
                index
                    .constructor_guards
                    .try_reserve_exact(1)
                    .map_err(|_| limit("constructor guard index allocation"))?;
                let guard = SlotId(index.slots);
                index.constructor_guards.push((guard, id));
                index.slots = next;
                Some(guard)
            } else {
                None
            };
            index
                .nodes
                .try_reserve(1)
                .map_err(|_| limit("body node allocation"))?;
            index.nodes.push(StructuralNode {
                id,
                parent: frame.parent,
                role,
                range: relative,
                scope,
                enclosing_syntax_target: target,
                value,
                place,
                operands: Vec::new(),
                runtime_temporaries,
                runtime_scalar_slots,
                runtime_scalar_phases,
                negative_integer_literal: expr.as_ref().is_some_and(negative_integer_literal),
                constructor_guard,
                tail_scope,
                condition_scope,
                clause_scope,
                boolean_scope,
                statement_scope,
                guard_scope,
                closure_scope,
                extended_to: None,
                opaque,
                pattern_statement: (frame.syntax.kind() == ra_ap_syntax::SyntaxKind::LET_STMT
                    && ast::LetStmt::cast(frame.syntax.clone())
                        .is_some_and(|statement| pattern_statement(&statement)))
                    || ast::ClosureExpr::cast(frame.syntax.clone())
                        .is_some_and(|closure| closure_runs_patterns(&closure))
                    || parameter_owner,
                parenthesized: frame.syntax.kind() == ra_ap_syntax::SyntaxKind::PAREN_EXPR,
            });
            if opaque {
                continue;
            }
            let mut children = Vec::new();
            for syntax in frame.syntax.children() {
                if index
                    .nodes
                    .len()
                    .checked_add(stack.len())
                    .and_then(|n| n.checked_add(children.len()))
                    .is_none_or(|n| n >= limits.nodes as usize)
                {
                    return Err(limit("called body node budget"));
                }
                charge(&mut index.estimated_bytes, 64, limits.metadata_bytes)?;
                children
                    .try_reserve(1)
                    .map_err(|_| limit("body traversal allocation"))?;
                let child_scope = if let Some(closure) = closure_scope {
                    closure
                } else if let Some(iteration) = for_iteration
                    && ast::Pat::can_cast(syntax.kind())
                {
                    iteration
                } else if ast::Expr::can_cast(syntax.kind()) {
                    statement_scope.unwrap_or(scope)
                } else {
                    scope
                };
                let child_expression = if kind.is_some()
                    || (role == NodeRole::Statement
                        && runtime_temporaries[0].is_some()
                        && ast::Expr::can_cast(syntax.kind()))
                {
                    Some(id)
                } else {
                    frame.expression
                };
                children.push(Frame {
                    syntax,
                    parent: Some(id),
                    expression: child_expression,
                    scope: Some(child_scope),
                    // Loop labels never cross a closure body.
                    target: if closure_scope.is_some() { None } else { target },
                });
            }
            stack
                .try_reserve(children.len())
                .map_err(|_| limit("body traversal allocation"))?;
            stack.extend(children.into_iter().rev());
        }
        let guard_spare_bytes = index
            .constructor_guards
            .capacity()
            .checked_sub(index.constructor_guards.len())
            .and_then(|spare| spare.checked_mul(std::mem::size_of::<(SlotId, NodeId)>()))
            .ok_or_else(|| limit("constructor guard metadata overflow"))?;
        charge(
            &mut index.estimated_bytes,
            guard_spare_bytes,
            limits.metadata_bytes,
        )?;
        let remaining = limits
            .metadata_bytes
            .checked_sub(index.estimated_bytes)
            .ok_or_else(|| limit("extension metadata budget"))?;
        let (annotations, bytes) =
            super::temporary_extensions::annotations(&root, body, &index, remaining)?;
        charge(&mut index.estimated_bytes, bytes, limits.metadata_bytes)?;
        for (node, target) in annotations {
            index.nodes[node.0 as usize].extended_to = Some(target);
        }
        if let Some(scope) = initializer_scope {
            if index.nodes.len() >= limits.nodes as usize {
                return Err(limit("initializer return node budget"));
            }
            let next_slots = index
                .slots
                .checked_add(2)
                .filter(|slots| *slots <= limits.slots)
                .ok_or_else(|| limit("initializer return slot budget"))?;
            charge(
                &mut index.estimated_bytes,
                std::mem::size_of::<StructuralNode>().max(192),
                limits.metadata_bytes,
            )?;
            let id = NodeId(
                u32::try_from(index.nodes.len())
                    .map_err(|_| limit("initializer return node ordinal overflow"))?,
            );
            let unit = SlotId(index.slots);
            index
                .nodes
                .try_reserve_exact(1)
                .map_err(|_| limit("initializer return metadata allocation"))?;
            let root_range = index
                .nodes
                .first()
                .ok_or_else(|| fault("initializer syntax root absent"))?
                .range;
            // Real source node IDs retain deterministic preorder. This one
            // administrative PC is appended after syntax and its annotations;
            // it is not a written operand or an inferred source expression.
            index.nodes.push(StructuralNode {
                id,
                parent: Some(NodeId(0)),
                role: NodeRole::InitializerReturn,
                range: root_range,
                scope,
                enclosing_syntax_target: None,
                value: None,
                place: None,
                operands: Vec::new(),
                runtime_temporaries: [Some(unit), None],
                runtime_scalar_slots: Vec::new(),
                runtime_scalar_phases: 0,
                negative_integer_literal: false,
                constructor_guard: None,
                tail_scope: None,
                condition_scope: None,
                clause_scope: None,
                boolean_scope: None,
                statement_scope: None,
                guard_scope: None,
                closure_scope: None,
                extended_to: None,
                opaque: true,
                pattern_statement: false,
                parenthesized: false,
            });
            index.slots = next_slots;
        }
        // Build this once, after real syntax and administrative IDs are final.
        // Lookup must not rebuild or sort ranges during phase preparation.
        let value_count = index.nodes.iter().filter(|node| node.value.is_some()).count();
        index
            .value_nodes
            .try_reserve_exact(value_count)
            .map_err(|_| limit("value node index allocation"))?;
        let value_bytes = index
            .value_nodes
            .capacity()
            .checked_mul(std::mem::size_of::<(SlotId, NodeId)>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<(SlotId, NodeId)>>()))
            .ok_or_else(|| limit("value node index metadata overflow"))?;
        charge(&mut index.estimated_bytes, value_bytes, limits.metadata_bytes)?;
        index.value_nodes.extend(index.nodes.iter().filter_map(|node| {
            node.value.map(|value| (value, node.id))
        }));
        index.value_nodes.sort_unstable_by_key(|(value, _)| value.0);
        if index.value_nodes.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(fault("primary value slot has multiple syntax owners"));
        }
        let row_bytes = index
            .nodes
            .len()
            .checked_mul(std::mem::size_of::<NodeId>())
            .ok_or_else(|| limit("source range index metadata overflow"))?;
        charge(&mut index.estimated_bytes, row_bytes, limits.metadata_bytes)?;
        index
            .source_order
            .try_reserve_exact(index.nodes.len())
            .map_err(|_| limit("source range index allocation"))?;
        let spare_bytes = index
            .source_order
            .capacity()
            .checked_sub(index.nodes.len())
            .and_then(|spare| spare.checked_mul(std::mem::size_of::<NodeId>()))
            .ok_or_else(|| limit("source range index capacity overflow"))?;
        charge(
            &mut index.estimated_bytes,
            spare_bytes,
            limits.metadata_bytes,
        )?;
        index
            .source_order
            .extend(index.nodes.iter().map(|node| node.id));
        let nodes = &index.nodes;
        index.source_order.sort_unstable_by_key(|id| {
            let node = &nodes[id.0 as usize];
            (node.range.start, node.range.end, id.0)
        });
        let remaining = limits
            .metadata_bytes
            .checked_sub(index.estimated_bytes)
            .filter(|remaining| {
                *remaining >= std::mem::size_of::<super::pattern_bindings::PatternBindings>()
            })
            .ok_or_else(|| limit("pattern binding metadata budget"))?;
        let bindings =
            super::pattern_bindings::PatternBindings::prepare(self, body, &index, remaining)
                .map_err(|error| {
                    if error.code == "LRR-E-METADATA-LIMIT" {
                        limit("pattern binding metadata budget")
                    } else {
                        error
                    }
                })?;
        charge(
            &mut index.estimated_bytes,
            bindings
                .metadata_bytes()
                .ok_or_else(|| limit("pattern binding metadata overflow"))?,
            limits.metadata_bytes,
        )?;
        index.pattern_bindings = bindings;
        index.semantic_identity = index.fingerprint();
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::digest::Digest;
    #[test]
    fn rustc_single_token_const_initializer_uses_exact_expression_scope() {
        use crate::source::snapshot::{CfgSet, SourceBlob};
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-as-fn.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([148; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "FOO")
            .unwrap();
        let body = item.body.as_ref().unwrap();
        let index = parsed.index_body(body, Default::default()).unwrap();
        let facts = parsed
            .entered_macro_facts(body, &CfgSet::guest_default(), &item.scope)
            .unwrap();
        assert_eq!(facts.scopes[0].range, body.range);
        assert!(facts.invocations.is_empty());
        assert_eq!(
            index.nodes()[0].role,
            NodeRole::Expression(ExpressionKind::Literal)
        );
        assert_eq!(index.scopes()[0].kind, ScopeKind::Initializer);
        assert_eq!(index.scopes()[0].parent, None);
        assert!(index.nodes()[0].constructor_guard.is_none());
        let same_range: Vec<_> = index
            .nodes_at_range(index.nodes()[0].range)
            .map(|node| (node.id, node.role))
            .collect();
        assert_eq!(
            same_range,
            vec![
                (NodeId(0), NodeRole::Expression(ExpressionKind::Literal)),
                (
                    index.nodes().last().unwrap().id,
                    NodeRole::InitializerReturn,
                ),
            ]
        );
        assert!(
            index
                .nodes_at_range(SourceRange {
                    start: u32::MAX,
                    end: u32::MAX,
                })
                .next()
                .is_none()
        );
        let exact_metadata_limit = BodyIndexLimits {
            metadata_bytes: index.estimated_bytes,
            ..Default::default()
        };
        assert_eq!(
            parsed
                .index_body(body, exact_metadata_limit)
                .unwrap()
                .fingerprint(),
            index.fingerprint()
        );
        let metadata_error = parsed
            .index_body(
                body,
                BodyIndexLimits {
                    metadata_bytes: index.estimated_bytes - 1,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert_eq!(metadata_error.code, "LRR-E-BODY-INDEX-LIMIT");
        assert_eq!(index.span(NodeId(0)).unwrap().body_key, body.key());
        assert_eq!(
            parsed
                .index_body(body, Default::default())
                .unwrap()
                .fingerprint(),
            index.fingerprint()
        );
        // The original compile-fail call FOO() is not selected or accepted here.
    }
    #[test]
    fn metadata_charge_rejects_overflow_and_preserves_the_previous_charge() {
        let mut bytes = 7;
        charge(&mut bytes, 1, 8).unwrap();
        assert_eq!(bytes, 8);
        assert!(charge(&mut bytes, 1, 8).is_err());
        assert_eq!(bytes, 8);
        let mut bytes = usize::MAX;
        assert!(charge(&mut bytes, 1, usize::MAX).is_err());
        assert_eq!(bytes, usize::MAX);
    }
    #[test]
    fn official_signed_radix_prefix_marker_is_exact_structural_and_encoded_all_editions() {
        use crate::source::snapshot::{CfgSet, SourceBlob};
        const SOURCE: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-radix.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(SOURCE.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let outline = parsed
                .outline(Digest([238; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let handle = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "main")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let index = parsed.index_body(handle, Default::default()).unwrap();
            let marked: Vec<_> = index
                .nodes()
                .iter()
                .filter(|node| node.negative_integer_literal)
                .collect();
            assert_eq!(marked.len(), 3);
            for (node, expected) in
                marked
                    .iter()
                    .zip(["-0xBEEF_isize", "-0o755_isize", "-0b10101_isize"])
            {
                assert_eq!(node.role, NodeRole::Expression(ExpressionKind::Other));
                assert_eq!(
                    &SOURCE[(handle.range.start + node.range.start) as usize
                        ..(handle.range.start + node.range.end) as usize],
                    expected
                );
                assert!(node.value.is_some());
                assert_eq!(index.pc(node.id, Phase::Entry).unwrap().phase, 0);
            }
            assert!(
                index
                    .nodes()
                    .iter()
                    .filter(|node| node.role != NodeRole::Expression(ExpressionKind::Other))
                    .all(|node| !node.negative_integer_literal)
            );
            let mut without_marker = index.clone();
            without_marker.nodes[marked[0].id.0 as usize].negative_integer_literal = false;
            assert_ne!(index.fingerprint(), without_marker.fingerprint());
            let mut old = StableEncoder::new("lrr/v3/original-body-structure");
            index.encode_fields(&mut old);
            assert_ne!(index.fingerprint(), old.finish());
        }
    }
}
