//! Reached ordinary scalar let conditions; no namespace, query or runtime effects.
use super::pattern_tests::{self as patterns, ScalarComparison, ScalarTestPlan};
use super::*;
use crate::ir::{op::PatternOperation, pc::SlotId};

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
#[cfg(test)]
pub(super) fn selected(pattern: &ast::Pat) -> bool {
    matches!(
        pattern,
        ast::Pat::LiteralPat(_)
            | ast::Pat::RangePat(_)
            | ast::Pat::IdentPat(_)
            | ast::Pat::WildcardPat(_)
    )
}

fn compare(
    comparison: ScalarComparison,
    source: SlotId,
    expected: SlotId,
    destination: SlotId,
) -> Vec<Op> {
    // The reached source supplies its runtime numeric constraint. Pattern-test
    // semantics and decoding are shared with ordinary match; this is only slot
    // context forwarding for a condition's independently indexed source.
    let mut operations = vec![Op::NumericHint {
        parent: destination,
        child: expected,
        left: Some(source),
        operator: Some(comparison.operator),
    }];
    operations.extend(comparison.operations(source, expected, destination, &HintRecipe::None));
    operations
}
#[derive(Debug)]
pub(super) struct Plan {
    id: NodeId,
    source_node: NodeId,
    pattern_node: NodeId,
    binding: Option<NodeId>,
    writable: bool,
    irrefutable: bool,
    scalar_test: Option<ScalarTestPlan>,
    hint: HintRecipe,
    next: PcKey,
    range: bool,
}
impl Plan {
    pub(super) fn requires_binding_admission(&self, key: &PcKey) -> bool {
        key.node == self.id.0
            && self.binding.is_some()
            && matches!(Phase::from_code(key.phase), Phase::Entry | Phase::Commit)
    }
}
pub(super) fn plan(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    expression: &ast::LetExpr,
) -> Result<Plan, Diagnostic> {
    let node = &index.nodes()[key.node as usize];
    let id = node.id;
    if key.body != body.key() || index.body() != &body.key() || index.profile() != &parsed.profile {
        return Err(error("condition owning source/profile differs"));
    }
    let pattern = expression
        .pat()
        .ok_or_else(|| error("condition pattern absent"))?;
    if pattern
        .syntax()
        .children()
        .any(|n| ast::Attr::can_cast(n.kind()))
    {
        return Err(unsupported(
            "condition pattern attributes require source admission",
        ));
    }
    let scrutinee = expression
        .expr()
        .ok_or_else(|| error("condition scrutinee absent"))?;
    let source_node = id_for(index, body, scrutinee.syntax())?;
    let range = matches!(pattern, ast::Pat::RangePat(_));
    let edition = match &pattern {
        ast::Pat::IdentPat(binding) => binding
            .name()
            .map(|name| {
                parsed.token_edition(crate::syntax::ast_adapter::range(
                    name.syntax().text_range(),
                ))
            })
            .transpose()?
            .unwrap_or(parsed.edition),
        _ => parsed.edition,
    };
    let irrefutable = match patterns::IrrefutablePattern::from_source(&pattern, edition) {
        Ok(header) => header,
        Err(error) => return Err(error),
    };
    let scalar_test = if irrefutable.is_none() {
        match ScalarTestPlan::from_source(&pattern) {
            Ok(test) => Some(test),
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let pattern_node = id_for(index, body, pattern.syntax())?;
    let hint = if irrefutable.is_some() {
        HintRecipe::None
    } else {
        patterns::source_hint(
            parsed,
            index,
            body,
            &scrutinee,
            patterns::source_type(&pattern),
            0,
        )?
    };
    let binding = irrefutable
        .as_ref()
        .and_then(|header| header.binding.as_ref())
        .map(|binding| id_for(index, body, binding.syntax()))
        .transpose()?;
    Ok(Plan {
        id,
        source_node,
        pattern_node,
        binding,
        writable: irrefutable.as_ref().is_some_and(|header| header.writable),
        irrefutable: irrefutable.is_some(),
        scalar_test,
        hint,
        next: return_pc(index, body, expression.syntax())?,
        range,
    })
}
#[cfg(test)]
pub(super) fn prepare(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    expression: &ast::LetExpr,
) -> Result<VerifiedRecipe, Diagnostic> {
    let retained = plan(parsed, body, index, key, expression);
    prepare_inner(
        parsed,
        body,
        index,
        key,
        retained.as_ref().map_err(Clone::clone),
        None,
    )
}
pub(super) fn prepare_inner(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    retained: Result<&Plan, Diagnostic>,
    admission: Option<&crate::frontend::scalar_pattern_binding::ScalarPatternBindingAdmission>,
) -> Result<VerifiedRecipe, Diagnostic> {
    let fallback = index.pc(NodeId(key.node), Phase::Exit)?;
    let refuse =
        |message: &str| thunks::unsupported(index, key.clone(), message.into(), fallback.clone());
    let plan = match retained {
        Ok(plan) => plan,
        Err(error) if error.code == "LRR-E-UNSUPPORTED" => return refuse(&error.message),
        Err(error) => return Err(error),
    };
    let Plan {
        id,
        source_node,
        pattern_node,
        binding,
        writable,
        irrefutable,
        scalar_test,
        hint,
        next,
        range,
    } = plan;
    let (id, source_node, pattern_node, range) = (*id, *source_node, *pattern_node, *range);
    let node = &index.nodes()[id.0 as usize];
    let source = index.nodes()[source_node.0 as usize]
        .value
        .ok_or_else(|| error("condition scrutinee value absent"))?;
    let expected =
        node.runtime_temporaries[0].ok_or_else(|| error("condition expected temporary absent"))?;
    let destination = node
        .value
        .ok_or_else(|| error("condition Bool result absent"))?;
    let phase = Phase::from_code(key.phase);
    let binding_admission = if plan.requires_binding_admission(key) {
        let Some(admission) = admission else {
            return refuse("pattern name requires current Value namespace binding admission");
        };
        admission.require(
            parsed,
            body,
            index,
            key,
            binding.ok_or_else(|| error("condition binding identity absent"))?,
        )?;
        Some(admission)
    } else {
        None
    };
    let draft = |index: &BodyIndex,
                 key: &PcKey,
                 operations: Vec<Op>,
                 exit: RecipeExit|
     -> Result<VerifiedRecipe, Diagnostic> {
        if let Some(admission) = binding_admission {
            literal_match::binding_draft(index, key, operations, exit, admission)
        } else {
            super::draft(index, key, operations, exit)
        }
    };
    let result = (|| match phase {
        Phase::Entry => draft(
            index,
            key,
            vec![
                Op::ScopeEnter {
                    scope: node
                        .clause_scope
                        .ok_or_else(|| error("condition clause scope absent"))?,
                },
                Op::SetHint {
                    child: source,
                    hint: hint.clone(),
                },
            ],
            RecipeExit::Next(index.pc(source_node, Phase::Entry)?),
        ),
        Phase::Exit => draft(
            index,
            key,
            vec![
                Op::Pattern {
                    operation: PatternOperation::Finish {
                        owner: id,
                        matched: Some(destination),
                    },
                },
                Op::Discard { source },
            ],
            RecipeExit::Next(next.clone()),
        ),
        Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&source_node) => {
            let mut operations = vec![Op::Pattern {
                operation: PatternOperation::Begin {
                    owner: id,
                    alternative: pattern_node,
                    source,
                    retry: index.pc(id, Phase::Exit)?,
                },
            }];
            let Some(scalar_test) = &scalar_test else {
                operations.push(Op::Literal {
                    literal: LiteralRecipe {
                        kind: LiteralKind::Bool,
                        spelling: "true".into(),
                        decoded: None,
                        source: index.span(id)?,
                    },
                    hint: HintRecipe::Primitive(PrimitiveType::Bool),
                    destination,
                });
                return draft(
                    index,
                    key,
                    operations,
                    RecipeExit::Next(index.pc(id, Phase::Commit)?),
                );
            };
            let comparison = scalar_test.first(parsed, body, key)?;
            let upper_pending = comparison.upper_pending;
            operations.extend(compare(comparison, source, expected, destination));
            draft(
                index,
                key,
                operations,
                if upper_pending {
                    RecipeExit::Branch {
                        condition: destination,
                        yes: index.pc(id, Phase::Commit)?,
                        no: index.pc(id, Phase::Exit)?,
                    }
                } else {
                    RecipeExit::Next(index.pc(id, Phase::Exit)?)
                },
            )
        }
        Phase::Commit if range => {
            let mut operations = vec![Op::Discard {
                source: destination,
            }];
            operations.extend(compare(
                scalar_test
                    .as_ref()
                    .ok_or_else(|| error("range test absent"))?
                    .upper(parsed, body, key)?,
                source,
                expected,
                destination,
            ));
            draft(
                index,
                key,
                operations,
                RecipeExit::Next(index.pc(id, Phase::Exit)?),
            )
        }
        Phase::Commit if *irrefutable => {
            let mut operations = Vec::new();
            if let Some(binding) = binding {
                operations.push(Op::Pattern {
                    operation: PatternOperation::StageBinding {
                        owner: id,
                        binding: *binding,
                        source,
                        writable: *writable,
                    },
                });
            }
            operations.push(Op::Pattern {
                operation: PatternOperation::Finish {
                    owner: id,
                    matched: None,
                },
            });
            if binding.is_none() {
                operations.push(Op::Discard { source });
            }
            draft(index, key, operations, RecipeExit::Next(next.clone()))
        }
        _ => Err(unsupported("invalid scalar condition phase")),
    })();
    match result {
        Err(error) if error.code == "LRR-E-UNSUPPORTED" => refuse(&error.message),
        other => other,
    }
}
#[cfg(test)]
#[path = "condition_ranges_tests.rs"]
mod tests;
