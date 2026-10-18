//! Runtime facts consume only type equalities already retained by the common
//! source component. No runtime values enter a pure query or source memo.
use super::{Component, FactKind, Step};
use crate::ir::op::{NumericTypeProjection, RelatedNumericPlan, RelatedNumericSource};
use std::sync::Arc;

impl Component<'_> {
    pub(super) fn related_numeric_plan(&self) -> Option<Arc<RelatedNumericPlan>> {
        let mut sources = Vec::new();
        for (node, via) in &self.expressions {
            if node.0 == self.key.node
                || !matches!(
                    self.plan.fact(*node).map(|fact| fact.kind()),
                    Some(
                        FactKind::Call
                            | FactKind::MethodCall
                            | FactKind::LangCall
                            | FactKind::Borrow
                            | FactKind::Path
                            | FactKind::Parenthesis
                            | FactKind::Prefix
                            | FactKind::Field
                            | FactKind::Index
                            | FactKind::IndexRange
                            | FactKind::Tuple
                            | FactKind::Array
                            | FactKind::If
                            | FactKind::Match
                            | FactKind::Block
                            | FactKind::Cast
                    )
                )
            {
                continue;
            }
            let projections = via
                .steps
                .iter()
                .map(|step| {
                    Some(match step {
                        Step::Deref => NumericTypeProjection::Deref,
                        Step::AutoDeref => NumericTypeProjection::AutoDeref,
                        Step::Field(field) => NumericTypeProjection::TupleField(*field),
                        Step::Element => NumericTypeProjection::Element,
                        Step::TypeArgument(position) => {
                            NumericTypeProjection::TypeArgument(*position)
                        }
                        // Source-selected fields/variants and trait projections must
                        // not become private runtime layout/name lookups.
                        Step::Named(_) | Step::Variant { .. } | Step::Item => return None,
                    })
                })
                .collect::<Option<Vec<_>>>();
            let Some(projections) = projections else {
                continue;
            };
            sources.push(RelatedNumericSource {
                node: *node,
                projections: projections.into_boxed_slice(),
            });
        }
        sources.sort();
        sources.dedup();
        (!sources.is_empty()).then(|| {
            Arc::new(RelatedNumericPlan {
                floating: self.floating,
                sources: sources.into_boxed_slice(),
            })
        })
    }
}
