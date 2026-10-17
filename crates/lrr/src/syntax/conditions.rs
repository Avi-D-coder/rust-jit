//! Pure original condition structure. No name/type resolution, thunk preparation,
//! pattern testing, guest mutation, expansion execution or compiler effects.
use super::{
    body_index::{BodyIndex, ExpressionKind, NodeRole},
    outline::BodyHandle,
    parse::{ParseKey, ParsedFile},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    ir::pc::NodeId,
    source::manifest::BodyKey,
};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};
use std::collections::BTreeMap;
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "condition plan preparation budget".into(),
    )
}
fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "let condition requires an unparenthesized top-level && clause".into(),
    )
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConditionLimits {
    pub clauses: usize,
    pub metadata_bytes: usize,
}
impl Default for ConditionLimits {
    fn default() -> Self {
        Self {
            clauses: usize::MAX,
            metadata_bytes: usize::MAX,
        }
    }
}
/// Structural clause boundaries shared with source lexical discovery. This
/// does not admit misplaced lets, resolve names or prepare executable shapes.
pub(crate) fn source_clauses(
    expression: ast::Expr,
    limits: ConditionLimits,
    bytes: &mut usize,
) -> Result<Vec<ast::Expr>, Diagnostic> {
    let mut pending = vec![expression];
    let mut clauses = Vec::new();
    while let Some(expression) = pending.pop() {
        if let ast::Expr::BinExpr(binary) = &expression
            && binary.op_token().is_some_and(|token| token.text() == "&&")
        {
            let left = binary
                .lhs()
                .ok_or_else(|| invalid("condition left operand absent"))?;
            let right = binary
                .rhs()
                .ok_or_else(|| invalid("condition right operand absent"))?;
            charge(bytes, 128, limits.metadata_bytes)?;
            pending.try_reserve(2).map_err(|_| limit())?;
            pending.push(right);
            pending.push(left);
        } else {
            if clauses.len() >= limits.clauses {
                return Err(limit());
            }
            charge(
                bytes,
                std::mem::size_of::<ast::Expr>(),
                limits.metadata_bytes,
            )?;
            clauses.try_reserve(1).map_err(|_| limit())?;
            clauses.push(expression);
        }
    }
    Ok(clauses)
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConditionClause {
    Boolean {
        node: NodeId,
    },
    Pattern {
        node: NodeId,
        pattern: NodeId,
        scrutinee: NodeId,
    },
}
impl ConditionClause {
    pub fn node(&self) -> NodeId {
        match self {
            Self::Boolean { node } | Self::Pattern { node, .. } => *node,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConditionPlan {
    source: ParseKey,
    body: BodyKey,
    conditional: NodeId,
    clauses: Vec<ConditionClause>,
}
impl ConditionPlan {
    pub fn clauses(&self) -> &[ConditionClause] {
        &self.clauses
    }
    pub fn require_source(&self, source: &ParseKey, body: &BodyKey) -> Result<(), Diagnostic> {
        if &self.source != source || &self.body != body {
            Err(invalid("condition plan source/profile/body differs"))
        } else {
            Ok(())
        }
    }
}
impl StableEncode for ConditionPlan {
    const DOMAIN: &'static str = "lrr/v2/original-condition-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.record(&self.body);
        e.u32(self.conditional.0);
        e.sequence(&self.clauses, |e, c| match c {
            ConditionClause::Boolean { node } => {
                e.u32(0);
                e.u32(node.0)
            }
            ConditionClause::Pattern {
                node,
                pattern,
                scrutinee,
            } => {
                e.u32(1);
                e.u32(node.0);
                e.u32(pattern.0);
                e.u32(scrutinee.0)
            }
        });
    }
}
fn charge(bytes: &mut usize, amount: usize, max: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= max)
        .ok_or_else(limit)?;
    Ok(())
}
fn range(node: &SyntaxNode) -> SourceRange {
    SourceRange {
        start: u32::from(node.text_range().start()),
        end: u32::from(node.text_range().end()),
    }
}
/// Inspect expression grammar only. Nested control/block bodies own their own
/// conditions; declarations and macro token trees are not parsed as clauses here.
fn misplaced_let(root: SyntaxNode, bytes: &mut usize, max: usize) -> Result<bool, Diagnostic> {
    let mut stack = vec![root];
    let mut visited = 0usize;
    while let Some(node) = stack.pop() {
        visited = visited
            .checked_add(1)
            .ok_or_else(limit)?;
        if ast::LetExpr::can_cast(node.kind()) {
            return Ok(true);
        }
        if ast::BlockExpr::can_cast(node.kind())
            || ast::IfExpr::can_cast(node.kind())
            || ast::WhileExpr::can_cast(node.kind())
            || ast::LoopExpr::can_cast(node.kind())
            || ast::ClosureExpr::can_cast(node.kind())
            || ast::MacroExpr::can_cast(node.kind())
            || ast::Item::can_cast(node.kind())
        {
            continue;
        }
        for child in node.children() {
            charge(bytes, 64, max)?;
            stack.try_reserve(1).map_err(|_| limit())?;
            stack.push(child)
        }
    }
    Ok(false)
}
impl ParsedFile {
    pub(crate) fn require_boolean_match_guard(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        node: NodeId,
    ) -> Result<(), Diagnostic> {
        if body.key() != *index.body() || self.profile != *index.profile() {
            return Err(invalid("match guard body/profile differs"));
        }
        let syntax = self
            .entered_node_syntax(body, index, node)?
            .filter(|syntax| ast::Expr::can_cast(syntax.kind()))
            .ok_or_else(|| invalid("match guard expression absent"))?;
        let mut bytes = 0;
        if misplaced_let(
            syntax,
            &mut bytes,
            ConditionLimits::default().metadata_bytes,
        )? {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "pattern and let-chain match guards are unsupported".into(),
            ));
        }
        Ok(())
    }
    pub fn condition_plan(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        conditional: NodeId,
        limits: ConditionLimits,
    ) -> Result<ConditionPlan, Diagnostic> {
        if limits.clauses == 0 || limits.metadata_bytes == 0 {
            return Err(limit());
        }
        if body.key() != *index.body() || self.profile != *index.profile() {
            return Err(invalid("condition plan body/profile differs"));
        }
        let selected = index
            .nodes()
            .get(conditional.0 as usize)
            .ok_or_else(|| invalid("condition plan node absent"))?;
        if !matches!(
            selected.role,
            NodeRole::Expression(ExpressionKind::If | ExpressionKind::While)
        ) {
            return Err(invalid("condition plan requires if or while"));
        }
        let syntax = self
            .entered_node_syntax(body, index, conditional)?
            .filter(|syntax| {
                matches!(
                    syntax.kind(),
                    ra_ap_syntax::SyntaxKind::IF_EXPR | ra_ap_syntax::SyntaxKind::WHILE_EXPR
                )
            })
            .ok_or_else(|| invalid("condition source node differs"))?;
        let expression = if let Some(x) = ast::IfExpr::cast(syntax.clone()) {
            x.condition()
        } else {
            syntax.children().find_map(ast::Expr::cast)
        }
        .ok_or_else(|| invalid("condition expression absent"))?;
        let mut bytes = std::mem::size_of::<ConditionPlan>()
            .checked_add(self.profile.semantics.capacity())
            .ok_or_else(limit)?;
        charge(&mut bytes, 0, limits.metadata_bytes)?;
        let mut locations = BTreeMap::new();
        for node in index.nodes() {
            let tag = match node.role {
                NodeRole::Expression(_) => 0,
                NodeRole::Pattern => 1,
                _ => continue,
            };
            charge(&mut bytes, 128, limits.metadata_bytes)?;
            if locations
                .insert((node.range.start, node.range.end, tag), node.id)
                .is_some()
            {
                return Err(invalid("duplicate condition syntax identity"));
            }
        }
        let id = |node: &SyntaxNode, tag: u8| -> Result<NodeId, Diagnostic> {
            let r = range(node);
            let start = r
                .start
                .checked_sub(body.range.start)
                .ok_or_else(|| invalid("condition node precedes body"))?;
            let end = r
                .end
                .checked_sub(body.range.start)
                .ok_or_else(|| invalid("condition node precedes body"))?;
            locations
                .get(&(start, end, tag))
                .copied()
                .ok_or_else(|| invalid("condition clause lacks structural identity"))
        };
        let sources = source_clauses(expression, limits, &mut bytes)?;
        let mut clauses = Vec::new();
        for expression in sources {
            let node = id(expression.syntax(), 0)?;
            let clause = if let ast::Expr::LetExpr(pattern) = expression {
                let pat = pattern
                    .pat()
                    .ok_or_else(|| invalid("condition pattern absent"))?;
                let scrutinee = pattern
                    .expr()
                    .ok_or_else(|| invalid("condition scrutinee absent"))?;
                ConditionClause::Pattern {
                    node,
                    pattern: id(pat.syntax(), 1)?,
                    scrutinee: id(scrutinee.syntax(), 0)?,
                }
            } else {
                if misplaced_let(
                    expression.syntax().clone(),
                    &mut bytes,
                    limits.metadata_bytes,
                )? {
                    return Err(unsupported());
                }
                ConditionClause::Boolean { node }
            };
            charge(&mut bytes, 64, limits.metadata_bytes)?;
            clauses.try_reserve(1).map_err(|_| limit())?;
            clauses.push(clause);
        }
        Ok(ConditionPlan {
            source: self.key(),
            body: body.key(),
            conditional,
            clauses,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::digest::Digest;
    use crate::{
        core::profile::ProfileIdentity,
        source::snapshot::{CfgSet, SourceBlob},
        syntax::body_index::ScopeKind,
    };
    #[test]
    fn rustc_const_if_initializer_preserves_its_unresolved_boolean_clause() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/basics.rs"
        );
        let cfg = CfgSet::guest_default();
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed.outline(Digest([150; 32]), vec![], &cfg).unwrap();
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "ABS_DIFF")
            .unwrap();
        let body = item.body.as_ref().unwrap();
        let index = parsed.index_body(body, Default::default()).unwrap();
        assert_eq!(index.scopes()[0].kind, ScopeKind::Initializer);
        assert_eq!(
            index.nodes()[0].role,
            NodeRole::Expression(ExpressionKind::If)
        );
        let plan = parsed
            .condition_plan(body, &index, NodeId(0), Default::default())
            .unwrap();
        let [ConditionClause::Boolean { node }] = plan.clauses() else {
            panic!("original const comparison clause is not retained")
        };
        let clause = &index.nodes()[node.0 as usize];
        let start = body.range.start.checked_add(clause.range.start).unwrap();
        let end = body.range.start.checked_add(clause.range.end).unwrap();
        assert_eq!(&ORIGINAL[start as usize..end as usize], "X < Y");
        plan.require_source(&parsed.key(), &body.key()).unwrap();
        let condition_scope = index.nodes()[0].condition_scope.unwrap();
        assert_eq!(
            index.scopes()[clause.scope.0 as usize].kind,
            ScopeKind::BooleanClause
        );
        assert_eq!(
            index.scopes()[clause.scope.0 as usize].parent,
            Some(condition_scope)
        );
        assert_eq!(
            index.scopes()[condition_scope.0 as usize].parent,
            Some(index.nodes()[0].scope)
        );
        // X, Y, the selected branch and the stock run-pass assertions remain
        // unevaluated. This is original structural condition preparation only.
    }
}
