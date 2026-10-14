//! Bounded syntactic let-extension annotations. No expression/type/name resolution.
use super::{
    body_index::{BodyIndex, NodeRole, ScopeId, ScopeKind},
    outline::BodyHandle,
};
use crate::{core::diagnostic::Diagnostic, ir::pc::NodeId};
use ra_ap_syntax::{
    AstNode, SyntaxNode, TextRange, TextSize,
    ast::{self, HasArgList},
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExtendedTo {
    pub scope: ScopeId,
    /// Every call on this frontier must resolve as a constructor before applying
    /// the annotation. Spelling/capitalization is never proof of construction.
    pub constructors: Vec<NodeId>,
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-BODY-INDEX-LIMIT",
        "temporary extension metadata budget".into(),
    )
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX", message.into())
}
#[derive(Clone, Copy)]
enum Mode {
    Frontier,
    Base,
}
struct Work {
    expression: ast::Expr,
    scope: ScopeId,
    constructors: Vec<NodeId>,
    mode: Mode,
}
struct Planner<'a> {
    body: &'a BodyHandle,
    index: &'a BodyIndex,
    ids: BTreeMap<(u32, u32), NodeId>,
    annotations: BTreeMap<NodeId, ExtendedTo>,
    pending: Vec<Work>,
    bytes: usize,
    maximum: usize,
}
impl Planner<'_> {
    fn charge(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.maximum)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn id(&self, expression: &ast::Expr) -> Result<NodeId, Diagnostic> {
        let r = expression.syntax().text_range();
        let start = u32::from(r.start())
            .checked_sub(self.body.range.start)
            .ok_or_else(|| invalid("extension expression precedes body"))?;
        let end = u32::from(r.end())
            .checked_sub(self.body.range.start)
            .ok_or_else(|| invalid("extension expression precedes body"))?;
        self.ids
            .get(&(start, end))
            .copied()
            .ok_or_else(|| invalid("extension expression lacks indexed identity"))
    }
    fn push(
        &mut self,
        expression: Option<ast::Expr>,
        scope: ScopeId,
        constructors: &[NodeId],
        mode: Mode,
    ) -> Result<(), Diagnostic> {
        if let Some(expression) = expression {
            self.charge(
                64usize
                    .checked_add(constructors.len().checked_mul(4).ok_or_else(limit)?)
                    .ok_or_else(limit)?,
            )?;
            let mut guards = Vec::new();
            guards
                .try_reserve_exact(constructors.len())
                .map_err(|_| limit())?;
            guards.extend_from_slice(constructors);
            self.pending.try_reserve(1).map_err(|_| limit())?;
            self.pending.push(Work {
                expression,
                scope,
                constructors: guards,
                mode,
            });
        }
        Ok(())
    }
    fn extending_pattern(&mut self, pattern: ast::Pat) -> Result<bool, Diagnostic> {
        let mut pending = vec![pattern];
        while let Some(pattern) = pending.pop() {
            self.charge(64)?;
            if let ast::Pat::IdentPat(binding) = &pattern {
                if binding.ref_token().is_some() {
                    return Ok(true);
                }
                continue;
            }
            // Explicit reference patterns are boundaries, not extending wrappers.
            let children: Box<dyn Iterator<Item = ast::Pat>> = match pattern {
                ast::Pat::ParenPat(p) => Box::new(p.syntax().children().filter_map(ast::Pat::cast)),
                ast::Pat::TuplePat(p) => Box::new(p.fields()),
                ast::Pat::TupleStructPat(p) => Box::new(p.fields()),
                ast::Pat::SlicePat(p) => Box::new(p.pats()),
                ast::Pat::OrPat(p) => Box::new(p.pats()),
                ast::Pat::RecordPat(p) => Box::new(
                    p.record_pat_field_list()
                        .into_iter()
                        .flat_map(|list| list.fields())
                        .filter_map(|field| field.pat()),
                ),
                _ => continue,
            };
            for child in children {
                self.charge(64)?;
                pending.try_reserve(1).map_err(|_| limit())?;
                pending.push(child);
            }
        }
        Ok(false)
    }
    fn finish(&mut self) -> Result<(), Diagnostic> {
        while let Some(work) = self.pending.pop() {
            let id = self.id(&work.expression)?;
            if matches!(work.mode, Mode::Base) {
                self.charge(
                    64usize
                        .checked_add(work.constructors.len().checked_mul(4).ok_or_else(limit)?)
                        .ok_or_else(limit)?,
                )?;
                let mut constructors = Vec::new();
                constructors
                    .try_reserve_exact(work.constructors.len())
                    .map_err(|_| limit())?;
                constructors.extend_from_slice(&work.constructors);
                let annotation = ExtendedTo {
                    scope: work.scope,
                    constructors,
                };
                if let Some(old) = self.annotations.get(&id) {
                    if old.scope != annotation.scope {
                        return Err(invalid("conflicting temporary extension scopes"));
                    }
                    if !old.constructors.is_empty() && annotation.constructors.is_empty() {
                        self.annotations.insert(id, annotation);
                    } else if old.constructors != annotation.constructors
                        && !old.constructors.is_empty()
                    {
                        return Err(invalid("conflicting constructor extension guards"));
                    }
                } else {
                    self.annotations.insert(id, annotation);
                }
            }
            if self.index.nodes()[id.0 as usize].opaque {
                continue;
            }
            let e = work.expression;
            let scope = work.scope;
            let guards = &work.constructors;
            match work.mode {
                Mode::Base => match e {
                    ast::Expr::RefExpr(e) if e.raw_token().is_none() => {
                        self.push(e.expr(), scope, guards, Mode::Base)?
                    }
                    ast::Expr::PrefixExpr(e) if e.op_kind() == Some(ast::UnaryOp::Deref) => {
                        self.push(e.expr(), scope, guards, Mode::Base)?
                    }
                    ast::Expr::FieldExpr(e) => self.push(e.expr(), scope, guards, Mode::Base)?,
                    ast::Expr::IndexExpr(e) => self.push(e.base(), scope, guards, Mode::Base)?,
                    ast::Expr::ParenExpr(e) => self.push(e.expr(), scope, guards, Mode::Base)?,
                    _ => {}
                },
                Mode::Frontier => match e {
                    ast::Expr::RefExpr(e) if e.raw_token().is_none() => {
                        self.push(e.expr(), scope, guards, Mode::Base)?
                    }
                    ast::Expr::ParenExpr(e) => {
                        self.push(e.expr(), scope, guards, Mode::Frontier)?
                    }
                    ast::Expr::CastExpr(e) => self.push(e.expr(), scope, guards, Mode::Frontier)?,
                    ast::Expr::TupleExpr(e) => {
                        for field in e.fields() {
                            self.push(Some(field), scope, guards, Mode::Frontier)?;
                        }
                    }
                    ast::Expr::ArrayExpr(e) if e.semicolon_token().is_none() => {
                        for element in e.exprs() {
                            self.push(Some(element), scope, guards, Mode::Frontier)?;
                        }
                    }
                    ast::Expr::RecordExpr(e) => {
                        if let Some(list) = e.record_expr_field_list() {
                            for field in list.fields() {
                                self.push(field.expr(), scope, guards, Mode::Frontier)?;
                            }
                        }
                    }
                    ast::Expr::BlockExpr(e) => {
                        self.push(e.tail_expr(), scope, guards, Mode::Frontier)?
                    }
                    ast::Expr::IfExpr(e) => {
                        self.push(
                            e.then_branch().and_then(|b| b.tail_expr()),
                            scope,
                            guards,
                            Mode::Frontier,
                        )?;
                        if let Some(branch) = e.else_branch() {
                            self.push(
                                ast::Expr::cast(branch.syntax().clone()),
                                scope,
                                guards,
                                Mode::Frontier,
                            )?;
                        }
                    }
                    ast::Expr::MatchExpr(e) => {
                        if let Some(list) = e.match_arm_list() {
                            for arm in list.arms() {
                                self.push(arm.expr(), scope, guards, Mode::Frontier)?;
                            }
                        }
                    }
                    ast::Expr::CallExpr(e) => {
                        self.charge(
                            guards
                                .len()
                                .checked_mul(4)
                                .and_then(|n| n.checked_add(64))
                                .ok_or_else(limit)?,
                        )?;
                        let mut next = Vec::new();
                        next.try_reserve_exact(guards.len().checked_add(1).ok_or_else(limit)?)
                            .map_err(|_| limit())?;
                        next.extend_from_slice(guards);
                        next.push(id);
                        if let Some(list) = e.arg_list() {
                            for argument in list.args() {
                                self.push(Some(argument), scope, &next, Mode::Frontier)?;
                            }
                        }
                    }
                    _ => {}
                },
            }
        }
        Ok(())
    }
}
pub(super) fn annotations(
    root: &SyntaxNode,
    body: &BodyHandle,
    index: &BodyIndex,
    maximum: usize,
) -> Result<(BTreeMap<NodeId, ExtendedTo>, usize), Diagnostic> {
    let mut planner = Planner {
        body,
        index,
        ids: BTreeMap::new(),
        annotations: BTreeMap::new(),
        pending: Vec::new(),
        bytes: 0,
        maximum,
    };
    let mut indexed = false;
    for node in index
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Statement)
    {
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(limit)?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(limit)?;
        let Some(syntax) = root
            .covering_element(TextRange::new(TextSize::from(start), TextSize::from(end)))
            .into_node()
        else {
            continue;
        };
        let Some(statement) = syntax
            .ancestors()
            .find(|n| {
                u32::from(n.text_range().start()) == start && u32::from(n.text_range().end()) == end
            })
            .and_then(ast::LetStmt::cast)
        else {
            continue;
        };
        let Some(initializer) = statement.initializer() else {
            continue;
        };
        if !indexed {
            for node in index
                .nodes()
                .iter()
                .filter(|n| matches!(n.role, NodeRole::Expression(_)))
            {
                planner.charge(128)?;
                if planner
                    .ids
                    .insert((node.range.start, node.range.end), node.id)
                    .is_some()
                {
                    return Err(invalid("duplicate extension expression identity"));
                }
            }
            indexed = true;
        }
        let mut containing = Some(node.scope);
        let mut scope = None;
        while let Some(id) = containing {
            let s = &index.scopes()[id.0 as usize];
            if s.kind == ScopeKind::Block {
                scope = Some(id);
                break;
            }
            containing = s.parent;
        }
        let scope = scope.ok_or_else(|| invalid("extending let lacks containing block"))?;
        if let Some(pattern) = statement.pat()
            && planner.extending_pattern(pattern)?
        {
            planner.push(Some(initializer.clone()), scope, &[], Mode::Base)?;
        }
        planner.push(Some(initializer), scope, &[], Mode::Frontier)?;
        planner.finish()?;
    }
    Ok((planner.annotations, planner.bytes))
}
