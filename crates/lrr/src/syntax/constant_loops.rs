//! Reached lexical control targets, privately derived from the retained pinned AST.
use super::*;
use ra_ap_syntax::ast::HasLoopBody;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Loop,
    Block,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlTarget(SourceRange, TargetKind);
impl ControlTarget {
    pub(super) fn loop_expression(expression: &ExprRef) -> Self {
        Self(expression.range(), TargetKind::Loop)
    }
    pub(super) fn block_expression(expression: &ast::BlockExpr) -> Self {
        Self(range(expression.syntax().text_range()), TargetKind::Block)
    }
    pub(crate) fn is_loop(self) -> bool {
        self.1 == TargetKind::Loop
    }
    #[cfg(test)]
    pub(super) fn range(self) -> SourceRange {
        self.0
    }
}
fn label_name(label: &ast::Lifetime) -> Result<String, Diagnostic> {
    let token = required(label.lifetime_ident_token())?;
    semantic(required(token.text().strip_prefix('\''))?)
}
pub(super) fn reached_target(
    syntax: &ra_ap_syntax::SyntaxNode,
    label: Option<ast::Lifetime>,
) -> Result<ControlTarget, Diagnostic> {
    resolve(syntax, label, false)
}
pub(super) fn reached_continue_target(
    syntax: &ra_ap_syntax::SyntaxNode,
    label: Option<ast::Lifetime>,
) -> Result<ControlTarget, Diagnostic> {
    resolve(syntax, label, true)
}
fn resolve(
    syntax: &ra_ap_syntax::SyntaxNode,
    label: Option<ast::Lifetime>,
    is_continue: bool,
) -> Result<ControlTarget, Diagnostic> {
    let wanted = label.as_ref().map(label_name).transpose()?;
    for parent in syntax.ancestors().skip(1) {
        if ast::Fn::can_cast(parent.kind())
            || ast::ClosureExpr::can_cast(parent.kind())
            || ast::Const::can_cast(parent.kind())
            || ast::Static::can_cast(parent.kind())
        {
            break;
        }
        if let Some(block) = ast::BlockExpr::cast(parent.clone()) {
            if block.const_token().is_some()
                || block.async_token().is_some()
                || block.gen_token().is_some()
            {
                break;
            }
            if let Some(label) = block.label() {
                let declared = label_name(&required(label.lifetime())?)?;
                let Some(wanted) = &wanted else {
                    return Err(fault("unlabelled constant exit crosses a labelled block"));
                };
                if &declared == wanted {
                    if is_continue {
                        return Err(fault("constant continue targets a labelled block"));
                    }
                    return Ok(ControlTarget::block_expression(&block));
                }
            }
        }
        let (is_loop, declared) = if let Some(expression) = ast::LoopExpr::cast(parent.clone()) {
            (true, expression.label())
        } else if let Some(expression) = ast::WhileExpr::cast(parent.clone()) {
            (true, expression.label())
        } else if let Some(expression) = ast::ForExpr::cast(parent.clone()) {
            (true, expression.label())
        } else {
            (false, None)
        };
        if !is_loop {
            continue;
        }
        if let Some(wanted) = &wanted {
            let Some(declared) = declared.and_then(|label| label.lifetime()) else {
                continue;
            };
            if label_name(&declared)? != *wanted {
                continue;
            }
        }
        return Ok(ControlTarget(range(parent.text_range()), TargetKind::Loop));
    }
    Err(fault(
        "constant exit has no lexical loop target in its source body",
    ))
}
