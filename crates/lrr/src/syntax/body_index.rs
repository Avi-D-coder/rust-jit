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
        digest::{Digest, StableEncode, StableEncoder},
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
            nodes: 100_000,
            slots: 200_000,
            metadata_bytes: 32 * 1024 * 1024,
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
    /// tests and initialized reference bindings.
    /// These reserve frame metadata only; no argument syntax or cell is prepared.
    pub runtime_temporaries: [Option<SlotId>; 2],
    /// Bounded token-count reservation for reached intrinsic scalar operands.
    /// Indexing does not parse or resolve the token-tree expressions.
    pub runtime_scalar_slots: Vec<SlotId>,
    pub runtime_scalar_phases: u16,
    /// Structural range-pattern continuation only; indexing never prepares bounds.
    pub scalar_range_phase: bool,
    /// A constructor's ownership guard is separate from its result cell.
    /// Reserving this slot never binds a type or allocates guest storage.
    pub constructor_guard: Option<SlotId>,
    pub tail_scope: Option<ScopeId>,
    pub condition_scope: Option<ScopeId>,
    pub clause_scope: Option<ScopeId>,
    pub boolean_scope: Option<ScopeId>,
    pub statement_scope: Option<ScopeId>,
    pub guard_scope: Option<ScopeId>,
    pub extended_to: Option<super::temporary_extensions::ExtendedTo>,
    pub opaque: bool,
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
    scopes: Vec<LexicalScope>,
    labels: Vec<LabelTarget>,
    constructor_guards: Vec<(SlotId, NodeId)>,
    slots: u32,
    estimated_bytes: usize,
}
impl BodyIndex {
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
            && !(node.role == NodeRole::Pattern && node.scalar_range_phase && n == 0)
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
    const DOMAIN: &'static str = "lrr/v3/original-body-structure";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.body);
        e.record(&self.profile);
        e.record(&self.cfg);
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
            e.bool(n.scalar_range_phase);
            e.option(&n.constructor_guard, |e, slot| e.u32(slot.0));
            e.option(&n.tail_scope, |e, scope| e.u32(scope.0));
            e.option(&n.condition_scope, |e, scope| e.u32(scope.0));
            e.option(&n.clause_scope, |e, scope| e.u32(scope.0));
            e.option(&n.boolean_scope, |e, scope| e.u32(scope.0));
            e.option(&n.statement_scope, |e, scope| e.u32(scope.0));
            e.option(&n.guard_scope, |e, scope| e.u32(scope.0));
            e.option(&n.extended_to, |e, target| {
                e.u32(target.scope.0);
                e.sequence(&target.constructors, |e, node| e.u32(node.0));
            });
            e.bool(n.opaque);
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
        _ => ExpressionKind::Other,
    }
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
            .take(257)
            .find(|node| range(node.text_range()) == body.range && ast::Expr::can_cast(node.kind()))
            .ok_or_else(|| fault("body expression syntax boundary absent"))?;
        let text = root.text();
        let mut content = String::new();
        content
            .try_reserve_exact(usize::from(text.len()))
            .map_err(|_| limit("body content verification allocation"))?;
        text.for_each_chunk(|chunk| content.push_str(chunk));
        if range(root.text_range()) != body.range
            || !ast::Expr::can_cast(root.kind())
            || Digest::of_bytes(content.as_bytes()) != body.body_hash
        {
            return Err(fault("body source range/content mismatch"));
        }
        if !ast::BlockExpr::can_cast(root.kind())
            && !root.parent().is_some_and(|parent| {
                ast::Const::can_cast(parent.kind()) || ast::Static::can_cast(parent.kind())
            })
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
        if limits.nodes > 100_000
            || limits.slots > 200_000
            || limits.metadata_bytes > 32 * 1024 * 1024
        {
            return Err(limit("called body fixed budget ceiling"));
        }
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
            scopes: Vec::new(),
            labels: Vec::new(),
            constructor_guards: Vec::new(),
            slots: 0,
            estimated_bytes: cfg_bytes,
        };
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
        let mut stack = vec![Frame {
            syntax: root.clone(),
            parent: None,
            expression: None,
            scope: initializer_scope,
            target: None,
        }];
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
                        ExpressionKind::Closure
                            | ExpressionKind::Async
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
                None
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
                    scope,
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
                    if operands.len() >= usize::from(u16::MAX - 2) {
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
            let runtime_temporaries = if kind == Some(ExpressionKind::Match) {
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
            } else if expr
                .as_ref()
                .is_some_and(|expr| matches!(expr, ast::Expr::LetExpr(_)))
            {
                let next = index
                    .slots
                    .checked_add(2)
                    .filter(|n| *n <= limits.slots)
                    .ok_or_else(|| limit("condition pattern temporary slot budget"))?;
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
                } else {
                    (Vec::new(), 0)
                };
            let constructor_guard = if expr.as_ref().is_some_and(|expr| {
                matches!(
                    expr,
                    ast::Expr::TupleExpr(_) | ast::Expr::ArrayExpr(_) | ast::Expr::RecordExpr(_)
                )
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
                scalar_range_phase: role == NodeRole::Pattern
                    && ast::RangePat::can_cast(frame.syntax.kind()),
                constructor_guard,
                tail_scope,
                condition_scope,
                clause_scope,
                boolean_scope,
                statement_scope,
                guard_scope,
                extended_to: None,
                opaque,
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
                let child_scope = if ast::Expr::can_cast(syntax.kind()) {
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
                    target,
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
                scalar_range_phase: false,
                constructor_guard: None,
                tail_scope: None,
                condition_scope: None,
                clause_scope: None,
                boolean_scope: None,
                statement_scope: None,
                guard_scope: None,
                extended_to: None,
                opaque: true,
            });
            index.slots = next_slots;
        }
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn range_phase_is_source_structural_encoded_and_exclusive_to_actual_range_nodes() {
        use crate::source::snapshot::{CfgSet, SourceBlob};
        const SOURCE: &str = "const RET:u8=1;fn chosen(c:u8){match c{0..=4=>(),_=>()}}";
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
                .outline(Digest([237; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let body = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "chosen")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let index = parsed.index_body(body, Default::default()).unwrap();
            assert_eq!(
                <BodyIndex as StableEncode>::DOMAIN,
                "lrr/v3/original-body-structure"
            );
            let ranges: Vec<_> = index
                .nodes()
                .iter()
                .filter(|node| node.scalar_range_phase)
                .collect();
            assert_eq!(ranges.len(), 1);
            let range = ranges[0];
            assert_eq!(range.role, NodeRole::Pattern);
            assert_eq!(
                &SOURCE[(body.range.start + range.range.start) as usize
                    ..(body.range.start + range.range.end) as usize],
                "0..=4"
            );
            assert!(range.operands.is_empty());
            assert_eq!(index.pc(range.id, Phase::Operand(0)).unwrap().phase, 3);
            assert!(index.pc(range.id, Phase::Operand(1)).is_err());
            for node in index.pattern_locations().filter(|node| node.id != range.id) {
                assert!(!node.scalar_range_phase);
                assert!(index.pc(node.id, Phase::Operand(0)).is_err());
            }
            let mut without_marker = index.clone();
            without_marker.nodes[range.id.0 as usize].scalar_range_phase = false;
            assert_ne!(index.fingerprint(), without_marker.fingerprint());
            assert!(without_marker.pc(range.id, Phase::Operand(0)).is_err());
            let initializer = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "RET")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let constant = parsed.index_body(initializer, Default::default()).unwrap();
            assert!(constant.nodes().iter().all(|node| !node.scalar_range_phase));
            let return_node = constant
                .nodes()
                .iter()
                .find(|node| node.role == NodeRole::InitializerReturn)
                .unwrap();
            assert!(constant.pc(return_node.id, Phase::Operand(0)).is_err());
        }
    }
}
