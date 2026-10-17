//! Closed source independence proof for a reached length under a fn header.
//! Parameter identifiers are potential shadows, never guessed Binding patterns.
use super::*;
use std::collections::BTreeSet;
#[derive(Debug, Clone)]
pub(super) struct ParameterIndependentLength {
    identity: Digest,
}
impl ParameterIndependentLength {
    pub(super) fn identity(&self) -> Digest {
        self.identity
    }
}
fn prerequisite(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-LENGTH-SOURCE", message.into())
}
fn metadata() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "parameter length source proof budget".into(),
    )
}
fn tick(work: &mut usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(1)
        .ok_or_else(metadata)?;
    Ok(())
}
fn insert(names: &mut BTreeSet<String>, name: String, bytes: &mut usize) -> Result<(), Diagnostic> {
    if names.contains(&name) {
        return Ok(());
    }
    *bytes = bytes
        .checked_add(128)
        .and_then(|n| n.checked_add(name.capacity()))
        .ok_or_else(metadata)?;
    names.insert(name);
    Ok(())
}
fn candidates(
    pattern: ast::Pat,
    names: &mut BTreeSet<String>,
    bytes: &mut usize,
    work: &mut usize,
) -> Result<(), Diagnostic> {
    for node in pattern.syntax().descendants() {
        tick(work)?;
        let Some(pat) = ast::Pat::cast(node) else {
            continue;
        };
        match pat {
            ast::Pat::IdentPat(p) => {
                let name = required(p.name())?;
                insert(names, semantic(name.text())?, bytes)?;
            }
            ast::Pat::TuplePat(_)
            | ast::Pat::SlicePat(_)
            | ast::Pat::ParenPat(_)
            | ast::Pat::RefPat(_)
            | ast::Pat::WildcardPat(_)
            | ast::Pat::RestPat(_) => {}
            _ => {
                return Err(prerequisite(
                    "parameter pattern requires its exact source binding context",
                ));
            }
        }
    }
    Ok(())
}
fn length_name(path: ast::Path) -> Result<String, Diagnostic> {
    bare(path)
}
impl ConstantSource {
    pub(super) fn prove_parameter_length(
        &self,
        function: &ast::Fn,
        expression: &ast::Expr,
    ) -> Result<ParameterIndependentLength, Diagnostic> {
        let params = required(function.param_list())?;
        if params.self_param().is_some()
            || function.generic_param_list().is_some()
            || function.where_clause().is_some()
        {
            return Err(prerequisite(
                "generic/self parameter length context is not established",
            ));
        }
        let mut work = 0usize;
        let mut bytes = 256usize;
        let mut parameters = BTreeSet::new();
        for param in params.params() {
            tick(&mut work)?;
            if param
                .syntax()
                .children()
                .any(|n| ast::Attr::can_cast(n.kind()))
            {
                return Err(prerequisite(
                    "parameter attributes require current prepared signature context",
                ));
            }
            candidates(
                required(param.pat())?,
                &mut parameters,
                &mut bytes,
                &mut work,
            )?;
        }
        let mut names = BTreeSet::new();
        let mut roots = BTreeSet::new();
        let mut stack = vec![expression.clone()];
        while let Some(expr) = stack.pop() {
            tick(&mut work)?;
            self.attributes_clear(expr.syntax())?;
            let mut operands = Vec::new();
            match expr {
                ast::Expr::Literal(_) => {}
                ast::Expr::ParenExpr(p) => operands.push(required(p.expr())?),
                ast::Expr::PrefixExpr(p) => {
                    if !matches!(p.op_kind(), Some(ast::UnaryOp::Neg | ast::UnaryOp::Not)) {
                        return Err(prerequisite(
                            "length reference/borrow operation is not source-independent",
                        ));
                    }
                    operands.push(required(p.expr())?);
                }
                ast::Expr::BinExpr(b) => {
                    if matches!(required(b.op_kind())?, ast::BinaryOp::Assignment { .. }) {
                        return Err(prerequisite(
                            "length assignment is not static source syntax",
                        ));
                    }
                    operands.push(required(b.lhs())?);
                    operands.push(required(b.rhs())?);
                }
                ast::Expr::CastExpr(c) => {
                    self.primitive(required(c.ty())?)?;
                    operands.push(required(c.expr())?);
                }
                ast::Expr::PathExpr(path) => {
                    let name = length_name(required(path.path())?)?;
                    self.length_root(
                        &name,
                        path.syntax(),
                        false,
                        &parameters,
                        &mut roots,
                        &mut bytes,
                    )?;
                    insert(&mut names, name, &mut bytes)?;
                }
                ast::Expr::CallExpr(call) => {
                    let ast::Expr::PathExpr(path) = required(call.expr())? else {
                        return Err(prerequisite(
                            "length callee requires an exact original module member",
                        ));
                    };
                    let name = length_name(required(path.path())?)?;
                    self.length_root(
                        &name,
                        call.syntax(),
                        true,
                        &parameters,
                        &mut roots,
                        &mut bytes,
                    )?;
                    insert(&mut names, name, &mut bytes)?;
                    for arg in required(call.arg_list())?.args() {
                        operands.push(arg);
                    }
                }
                ast::Expr::MacroExpr(m) => {
                    // Only already privately admitted concrete panic semantics
                    // has proven static operands; no macro execution or spelling.
                    if !matches!(
                        self.node(&ExprRef(ast::Expr::MacroExpr(m)))?,
                        Node::AdmittedPanic(_)
                    ) {
                        return Err(prerequisite(
                            "length macro has no current source-static operand proof",
                        ));
                    }
                }
                _ => {
                    return Err(prerequisite(
                        "length expression requires additional source local/control context",
                    ));
                }
            }
            stack
                .len()
                .checked_add(operands.len())
                .and_then(|n| n.checked_mul(std::mem::size_of::<ast::Expr>()))
                .ok_or_else(metadata)?;
            stack
                .try_reserve_exact(operands.len())
                .map_err(|_| metadata())?;
            stack.extend(operands);
        }
        // Do not inspect prior initializer/callee bodies. Only names from the
        // exact selected expression need preceding local-pattern exclusion.
        for ancestor in expression.syntax().ancestors() {
            if let Some(statements) = ast::StmtList::cast(ancestor) {
                for statement in statements.statements() {
                    tick(&mut work)?;
                    if let ast::Stmt::LetStmt(binding) = statement
                        && range(binding.syntax().text_range()).end
                            <= range(expression.syntax().text_range()).start
                    {
                        let mut locals = BTreeSet::new();
                        candidates(required(binding.pat())?, &mut locals, &mut bytes, &mut work)?;
                        if !locals.is_disjoint(&names) {
                            return Err(prerequisite(
                                "length path may capture a preceding local instead of its module constant",
                            ));
                        }
                    }
                }
            }
        }
        let declaration = self.declaration(function)?;
        let header_start = range(function.syntax().text_range()).start;
        let header_end = range(required(function.body())?.syntax().text_range()).start;
        let header = SourceRange::checked(header_start, header_end)?;
        let width = header.end.checked_sub(header.start).ok_or_else(metadata)?;
        if width > u32::from(function.syntax().text_range().len()) {
            return Err(prerequisite(
                "exact function header source range is invalid",
            ));
        }
        bytes
            .checked_add(width as usize)
            .ok_or_else(metadata)?;
        // Retain the existing immutable source lease; copy only the bounded
        // original header, never the complete source or unentered body text.
        let text = function
            .syntax()
            .text()
            .slice(..ra_ap_syntax::TextSize::from(width))
            .to_string();
        let mut e = StableEncoder::new("lrr/v1/parameter-independent-constant-length");
        e.digest(declaration.definition());
        e.record(declaration.context());
        e.record(self.profile());
        e.digest(self.cfg());
        e.u32(header.start);
        e.u32(header.end);
        e.digest(Digest::of_bytes(text.as_bytes()));
        let site = range(expression.syntax().text_range());
        e.u32(site.start);
        e.u32(site.end);
        e.u64(parameters.len() as u64);
        for name in &parameters {
            e.string(name);
        }
        e.u64(names.len() as u64);
        for name in &names {
            e.string(name);
        }
        e.u64(roots.len() as u64);
        for root in &roots {
            e.digest(*root);
        }
        Ok(ParameterIndependentLength {
            identity: e.finish(),
        })
    }
    fn length_root(
        &self,
        name: &str,
        syntax: &ra_ap_syntax::SyntaxNode,
        call: bool,
        parameters: &BTreeSet<String>,
        roots: &mut BTreeSet<Digest>,
        bytes: &mut usize,
    ) -> Result<(), Diagnostic> {
        if parameters.contains(name) {
            return Err(prerequisite(
                "length path may read an owning function parameter",
            ));
        }
        self.lexical_value_clear(syntax, name)?;
        let item = self.named(name)?;
        self.attributes_clear(item.syntax())?;
        let admitted = match &item {
            ast::Item::Const(_) => !call,
            ast::Item::Fn(f) => {
                call && f.const_token().is_some()
                    && f.generic_param_list().is_none()
                    && f.where_clause().is_none()
            }
            _ => false,
        };
        if !admitted {
            return Err(prerequisite(
                "length path is not an exact supported static module member",
            ));
        }
        // Structural kind/context only: no callee type/body semantics forced.
        let declaration = self.declaration(&item)?;
        if declaration.context().scope != self.0.scope {
            return Err(prerequisite(
                "length root member belongs to another source namespace",
            ));
        }
        if !roots.contains(&declaration.definition()) {
            *bytes = bytes
                .checked_add(128)
                .ok_or_else(metadata)?;
            roots.insert(declaration.definition());
        }
        Ok(())
    }
}
