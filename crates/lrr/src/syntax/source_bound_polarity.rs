//! Polarity of the retained ordinary TypeBound, independent of its consumer.
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, WalkEvent, ast};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceBoundPolarity {
    Positive,
    Relaxed,
    Negative,
    Lifetime,
}

/// Inspect only direct modifier tokens; a nested type's `!` is not polarity.
/// Qualification/binder transport remains the owning consumer's responsibility.
pub(crate) fn polarity(bound: &ast::TypeBound) -> SourceBoundPolarity {
    if bound
        .syntax()
        .children_with_tokens()
        .any(|token| token.kind() == SyntaxKind::BANG)
    {
        SourceBoundPolarity::Negative
    } else if bound.question_mark_token().is_some() {
        SourceBoundPolarity::Relaxed
    } else if bound.lifetime().is_some() {
        SourceBoundPolarity::Lifetime
    } else {
        SourceBoundPolarity::Positive
    }
}

/// Preserve the existing principal-selection rule without treating a negative
/// or relaxed row as positive, or hiding a negative row after the principal.
pub(crate) fn positive_principal(bounds: Option<ast::TypeBoundList>) -> Option<ast::PathType> {
    let mut principal = None;
    for bound in bounds.into_iter().flat_map(|bounds| bounds.bounds()) {
        match polarity(&bound) {
            SourceBoundPolarity::Negative => return None,
            SourceBoundPolarity::Positive => {
                if principal.is_none()
                    && let Some(ast::Type::PathType(path)) = bound.ty()
                {
                    principal = Some(path);
                }
            }
            _ => {}
        }
    }
    principal
}

/// Header metadata only: do not inspect executable blocks or other members.
pub(crate) fn negative_in_header(declaration: &SyntaxNode) -> bool {
    let mut walk = declaration.preorder();
    while let Some(event) = walk.next() {
        let WalkEvent::Enter(node) = event else {
            continue;
        };
        // Const-generic/header types may contain blocks with cold local items.
        // Prune at every depth, not only the declaration's direct children.
        if ast::BlockExpr::can_cast(node.kind()) || ast::AssocItemList::can_cast(node.kind()) {
            walk.skip_subtree();
            continue;
        }
        if let Some(bound) = ast::TypeBound::cast(node)
            && polarity(&bound) == SourceBoundPolarity::Negative
        {
            return true;
        }
    }
    false
}
