//! Pure, retained structural facts for demand-driven source type constraints.
//! This table grants no name/type authority and prepares no executable children.
use super::{
    body_index::{BodyIndex, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    ir::pc::NodeId,
    source::manifest::BodyKey,
};
use ra_ap_syntax::{
    AstNode, AstToken, SyntaxKind, SyntaxNode,
    ast::{self, HasArgList},
};
use std::collections::BTreeMap;

const MAX_NODES: usize = 100_000;
const MAX_EDGES: usize = 200_000;
const MAX_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const SOURCE_CONSTRAINT_SCHEMA: u32 = 2;
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "source constraint plan budget".into(),
    )
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX", message.into())
}

/// Syntax categories only. For example Binary does not assert primitive
/// arithmetic, and Path does not certify either a binding or an item.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FactKind {
    Opaque,
    Path,
    Integer,
    Float,
    Literal,
    Parenthesis,
    Prefix,
    Binary,
    Call,
    MethodCall,
    Let,
    Block,
    Return,
    Cast,
    Borrow,
    SuffixedInteger,
    SuffixedFloat,
}
impl FactKind {
    fn tag(self) -> u8 {
        match self {
            Self::Opaque => 0,
            Self::Path => 1,
            Self::Integer => 2,
            Self::Float => 3,
            Self::Literal => 4,
            Self::Parenthesis => 5,
            Self::Prefix => 6,
            Self::Binary => 7,
            Self::Call => 8,
            Self::MethodCall => 9,
            Self::Let => 10,
            Self::Block => 11,
            Self::Return => 12,
            Self::Cast => 13,
            Self::Borrow => 14,
            Self::SuffixedInteger => 15,
            Self::SuffixedFloat => 16,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SourceFact {
    kind: FactKind,
    // Preserve missing children and their ordinal. A Call's first edge is the
    // callee; a MethodCall's first edge is the receiver. Let uses pattern, init.
    operands: Vec<Option<NodeId>>,
    annotation: Option<SourceRange>,
    // Exact operator token location, not its printed text or a type judgment.
    operator: Option<SourceRange>,
    numeric_prefix: bool,
}

/// Derived reverse edges permit a demand to visit only its connected syntax.
/// Ordinals retain each parent's semantic position without an AST search.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct SourceUse {
    pub(crate) child: NodeId,
    pub(crate) parent: NodeId,
    pub(crate) ordinal: u32,
}
impl SourceFact {
    pub(crate) fn kind(&self) -> FactKind {
        self.kind
    }
    pub(crate) fn operands(&self) -> &[Option<NodeId>] {
        &self.operands
    }
    pub(crate) fn annotation(&self) -> Option<SourceRange> {
        self.annotation
    }
    pub(crate) fn operator(&self) -> Option<SourceRange> {
        self.operator
    }
    pub(crate) fn numeric_prefix(&self) -> bool {
        self.numeric_prefix
    }
    fn opaque() -> Self {
        Self {
            kind: FactKind::Opaque,
            operands: Vec::new(),
            annotation: None,
            operator: None,
            numeric_prefix: false,
        }
    }
}

/// Owned compact facts, with no parser/index/environment Arc to deep-copy or
/// retire. Real source dependencies are retained by the existing query memo.
#[derive(Debug, Clone)]
pub(crate) struct SourceConstraintPlan {
    body: BodyKey,
    structure: Digest,
    facts: Vec<SourceFact>,
    uses: Vec<SourceUse>,
    path_names: BTreeMap<String, Vec<NodeId>>,
    bytes: usize,
}
impl SourceConstraintPlan {
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn structure(&self) -> Digest {
        self.structure
    }
    pub(crate) fn facts(&self) -> &[SourceFact] {
        &self.facts
    }
    pub(crate) fn fact(&self, node: NodeId) -> Option<&SourceFact> {
        self.facts.get(node.0 as usize)
    }
    pub(crate) fn uses(&self, node: NodeId) -> &[SourceUse] {
        let start = self.uses.partition_point(|edge| edge.child < node);
        let count = self.uses[start..].partition_point(|edge| edge.child == node);
        &self.uses[start..start + count]
    }
    /// Candidate filter only. Equal spelling does not establish a binding;
    /// every selected use still requires the common hygienic Value goal.
    pub(crate) fn paths_named(&self, name: &str) -> &[NodeId] {
        self.path_names
            .get(name.strip_prefix("r#").unwrap_or(name))
            .map_or(&[], Vec::as_slice)
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn prepare(
        parsed: &ParsedFile,
        body: &BodyHandle,
        index: &BodyIndex,
    ) -> Result<Self, Diagnostic> {
        if index.body() != &body.key() || index.profile() != &parsed.profile {
            return Err(invalid("source constraint body/profile differs"));
        }
        // Genuine entry validation once. Subsequent attachment follows retained
        // indexed locations, not a whole-file descendants search per node.
        parsed.entered_body_root(body)?;
        if index.nodes().len() > MAX_NODES {
            return Err(limit());
        }
        let mut facts = Vec::new();
        facts
            .try_reserve_exact(index.nodes().len())
            .map_err(|_| limit())?;
        let mut bytes = facts
            .capacity()
            .checked_mul(std::mem::size_of::<SourceFact>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(limit)?;
        if bytes > MAX_BYTES {
            return Err(limit());
        }
        let mut edges = 0usize;
        let mut path_names = BTreeMap::new();
        for node in index.nodes() {
            if node.id.0 as usize != facts.len() {
                return Err(invalid("source constraint node order differs"));
            }
            let mut fact = SourceFact::opaque();
            if matches!(node.role, NodeRole::Expression(_) | NodeRole::Statement)
                && let Some(syntax) = parsed.entered_node_syntax(body, index, node.id)?
            {
                if let Some(name) = unqualified_path_name(&syntax) {
                    index_name(&mut path_names, name.text(), node.id, &mut bytes)?;
                }
                classify(body, index, syntax, &mut fact, &mut edges)?;
            }
            bytes = bytes
                .checked_add(
                    fact.operands
                        .capacity()
                        .checked_mul(std::mem::size_of::<Option<NodeId>>())
                        .ok_or_else(limit)?,
                )
                .filter(|n| *n <= MAX_BYTES)
                .ok_or_else(limit)?;
            facts.push(fact);
        }
        let mut uses = Vec::new();
        uses.try_reserve_exact(edges).map_err(|_| limit())?;
        bytes = bytes
            .checked_add(
                uses.capacity()
                    .checked_mul(std::mem::size_of::<SourceUse>())
                    .ok_or_else(limit)?,
            )
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        for (parent, fact) in facts.iter().enumerate() {
            let parent = NodeId(u32::try_from(parent).map_err(|_| limit())?);
            for (ordinal, child) in fact.operands.iter().enumerate() {
                if let Some(child) = child {
                    if child.0 as usize >= facts.len() {
                        return Err(invalid("source constraint edge node absent"));
                    }
                    uses.push(SourceUse {
                        child: *child,
                        parent,
                        ordinal: u32::try_from(ordinal).map_err(|_| limit())?,
                    });
                }
            }
        }
        uses.sort_unstable();
        Ok(Self {
            body: body.key(),
            structure: index.semantic_identity(),
            facts,
            uses,
            path_names,
            bytes,
        })
    }
}

fn unqualified_path_name(syntax: &SyntaxNode) -> Option<ast::NameRef> {
    let path = ast::PathExpr::cast(syntax.clone())?.path()?;
    if path.qualifier().is_some() {
        return None;
    }
    path.segment()?.name_ref()
}
fn index_name(
    names: &mut BTreeMap<String, Vec<NodeId>>,
    raw: &str,
    node: NodeId,
    bytes: &mut usize,
) -> Result<(), Diagnostic> {
    let name = raw.strip_prefix("r#").unwrap_or(raw);
    if name.len() > 64 * 1024 {
        return Err(limit());
    }
    if !names.contains_key(name) {
        // Conservative deterministic BTree node allowance. No string is made
        // before bounding its size; source facts never intern guest state.
        let overhead = std::mem::size_of::<(String, Vec<NodeId>)>()
            .checked_add(128)
            .ok_or_else(limit)?;
        *bytes = bytes
            .checked_add(overhead)
            .and_then(|n| n.checked_add(name.len()))
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        let mut key = String::new();
        key.try_reserve_exact(name.len()).map_err(|_| limit())?;
        key.push_str(name);
        *bytes = bytes
            .checked_add(key.capacity().checked_sub(name.len()).ok_or_else(limit)?)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        names.insert(key, Vec::new());
    }
    let row = names
        .get_mut(name)
        .ok_or_else(|| invalid("source path-name index absent"))?;
    let old = row.capacity();
    row.try_reserve(1).map_err(|_| limit())?;
    let growth = row
        .capacity()
        .checked_sub(old)
        .and_then(|n| n.checked_mul(std::mem::size_of::<NodeId>()))
        .ok_or_else(limit)?;
    *bytes = bytes
        .checked_add(growth)
        .filter(|n| *n <= MAX_BYTES)
        .ok_or_else(limit)?;
    row.push(node);
    Ok(())
}

fn relative(body: &BodyHandle, range: ra_ap_syntax::TextRange) -> Result<SourceRange, Diagnostic> {
    let absolute = super::ast_adapter::range(range);
    if !body.range.contains(absolute) {
        return Err(invalid("source constraint child outside body"));
    }
    Ok(SourceRange {
        start: absolute
            .start
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
        end: absolute
            .end
            .checked_sub(body.range.start)
            .ok_or_else(limit)?,
    })
}
fn edge(
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: Option<SyntaxNode>,
    fact: &mut SourceFact,
    edges: &mut usize,
) -> Result<(), Diagnostic> {
    *edges = edges
        .checked_add(1)
        .filter(|n| *n <= MAX_EDGES)
        .ok_or_else(limit)?;
    let node = if let Some(syntax) = syntax {
        let range = relative(body, syntax.text_range())?;
        index
            .nodes_at_range(range)
            .find(|node| {
                if ast::Expr::can_cast(syntax.kind()) {
                    matches!(node.role, NodeRole::Expression(_))
                } else if ast::Pat::can_cast(syntax.kind()) {
                    node.role == NodeRole::Pattern
                } else {
                    false
                }
            })
            .map(|node| node.id)
    } else {
        None
    };
    fact.operands.try_reserve(1).map_err(|_| limit())?;
    fact.operands.push(node);
    Ok(())
}
fn expr_edge(
    body: &BodyHandle,
    index: &BodyIndex,
    expression: Option<ast::Expr>,
    fact: &mut SourceFact,
    edges: &mut usize,
) -> Result<(), Diagnostic> {
    edge(
        body,
        index,
        expression.map(|e| e.syntax().clone()),
        fact,
        edges,
    )
}
fn classify(
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: SyntaxNode,
    fact: &mut SourceFact,
    edges: &mut usize,
) -> Result<(), Diagnostic> {
    if let Some(statement) = ast::LetStmt::cast(syntax.clone()) {
        fact.kind = FactKind::Let;
        edge(
            body,
            index,
            statement.pat().map(|p| p.syntax().clone()),
            fact,
            edges,
        )?;
        expr_edge(body, index, statement.initializer(), fact, edges)?;
        fact.annotation = statement
            .ty()
            .map(|ty| relative(body, ty.syntax().text_range()))
            .transpose()?;
        return Ok(());
    }
    let Some(expression) = ast::Expr::cast(syntax) else {
        return Ok(());
    };
    match expression {
        ast::Expr::PathExpr(_) => fact.kind = FactKind::Path,
        ast::Expr::Literal(literal) => {
            fact.kind = match literal.token().kind() {
                SyntaxKind::INT_NUMBER => match ast::IntNumber::cast(literal.token()) {
                    Some(number) if number.suffix().is_none() => FactKind::Integer,
                    Some(_) => FactKind::SuffixedInteger,
                    None => FactKind::Literal,
                },
                SyntaxKind::FLOAT_NUMBER => match ast::FloatNumber::cast(literal.token()) {
                    Some(number) if number.suffix().is_none() => FactKind::Float,
                    Some(_) => FactKind::SuffixedFloat,
                    None => FactKind::Literal,
                },
                _ => FactKind::Literal,
            }
        }
        ast::Expr::ParenExpr(group) => {
            fact.kind = FactKind::Parenthesis;
            expr_edge(body, index, group.expr(), fact, edges)?;
        }
        ast::Expr::PrefixExpr(prefix) => {
            fact.kind = FactKind::Prefix;
            fact.numeric_prefix = prefix
                .op_token()
                .is_some_and(|token| matches!(token.text(), "-" | "!"));
            fact.operator = prefix
                .op_token()
                .map(|t| relative(body, t.text_range()))
                .transpose()?;
            expr_edge(body, index, prefix.expr(), fact, edges)?;
        }
        ast::Expr::BinExpr(binary) => {
            fact.kind = FactKind::Binary;
            fact.operator = binary
                .op_token()
                .map(|t| relative(body, t.text_range()))
                .transpose()?;
            expr_edge(body, index, binary.lhs(), fact, edges)?;
            expr_edge(body, index, binary.rhs(), fact, edges)?;
        }
        ast::Expr::CallExpr(call) => {
            fact.kind = FactKind::Call;
            expr_edge(body, index, call.expr(), fact, edges)?;
            if let Some(args) = call.arg_list() {
                for argument in args.args() {
                    expr_edge(body, index, Some(argument), fact, edges)?;
                }
            }
        }
        ast::Expr::MethodCallExpr(call) => {
            fact.kind = FactKind::MethodCall;
            expr_edge(body, index, call.receiver(), fact, edges)?;
            if let Some(args) = call.arg_list() {
                for argument in args.args() {
                    expr_edge(body, index, Some(argument), fact, edges)?;
                }
            }
        }
        ast::Expr::BlockExpr(block) => {
            fact.kind = FactKind::Block;
            expr_edge(
                body,
                index,
                block.stmt_list().and_then(|s| s.tail_expr()),
                fact,
                edges,
            )?;
        }
        ast::Expr::ReturnExpr(ret) => {
            fact.kind = FactKind::Return;
            expr_edge(body, index, ret.expr(), fact, edges)?;
        }
        ast::Expr::CastExpr(cast) => {
            fact.kind = FactKind::Cast;
            expr_edge(body, index, cast.expr(), fact, edges)?;
            fact.annotation = cast
                .ty()
                .map(|ty| relative(body, ty.syntax().text_range()))
                .transpose()?;
        }
        ast::Expr::RefExpr(reference) => {
            fact.kind = FactKind::Borrow;
            expr_edge(body, index, reference.expr(), fact, edges)?;
        }
        // Incomplete source relationships are opaque until demanded by the
        // shared solver. Never lower unselected branches to populate this table.
        _ => {}
    }
    Ok(())
}

impl StableEncode for SourceConstraintPlan {
    const DOMAIN: &'static str = "lrr/v1/source-constraint-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(SOURCE_CONSTRAINT_SCHEMA);
        e.record(&self.body);
        e.digest(self.structure);
        e.sequence(&self.facts, |e, fact| {
            e.u8(fact.kind.tag());
            e.sequence(&fact.operands, |e, node| e.option(node, |e, n| e.u32(n.0)));
            e.option(&fact.annotation, encode_range);
            e.option(&fact.operator, encode_range);
            e.bool(fact.numeric_prefix);
        });
    }
}
fn encode_range(e: &mut StableEncoder, range: &SourceRange) {
    e.u32(range.start);
    e.u32(range.end);
}
