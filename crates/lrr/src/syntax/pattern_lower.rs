//! Ordinary indexed leaf recipes over the one retained pattern graph.
//! Owners provide scrutinee/guard/result continuations; leaf semantics are shared.
use super::*;
use crate::{
    frontend::thunks,
    ir::{
        op::{HintRecipe, Op, PatternDecision, PatternOperation},
        recipe::{RecipeExit, VerifiedRecipe},
    },
};

impl PatternProductPlan {
    pub(crate) fn lower_node(
        self: &Arc<Self>,
        parsed: &ParsedFile,
        body: &BodyHandle,
        key: &PcKey,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.require_source(parsed, body)?;
        self.require_runtime(&key.body, &parsed.profile)?;
        let index = self.index();
        let node = NodeId(key.node);
        let row = self
            .row(node)
            .ok_or_else(|| invalid("pattern decision source row absent"))?;
        let owner = &index.nodes()[self.owner().0 as usize];
        let source = index.nodes()[node.0 as usize].runtime_temporaries[0]
            .ok_or_else(|| invalid("pattern decision value scratch absent"))?;
        let expected = owner.runtime_temporaries[0]
            .ok_or_else(|| invalid("pattern expected scratch absent"))?;
        let test = if self.arm().is_some() {
            owner.runtime_temporaries[1]
        } else {
            owner.value
        }
        .ok_or_else(|| invalid("pattern comparison Bool scratch absent"))?;
        let operation = |transition| Op::Pattern {
            operation: PatternOperation::Decision {
                owner: self.owner(),
                plan: Arc::clone(self),
                transition,
            },
        };
        let emit = |operations| {
            super::super::draft(
                index,
                key,
                operations,
                RecipeExit::PatternNext {
                    owner: self.owner(),
                },
            )
        };
        let refuse = |message: &str| {
            thunks::unsupported(
                index,
                key.clone(),
                message.into(),
                index.pc(node, Phase::Exit)?,
            )
        };
        let phase = Phase::from_code(key.phase);
        if let DecisionKind::Unsupported(error) = &row.kind {
            return refuse(&error.message);
        }
        match phase {
            Phase::Entry => {
                let mut operations = Vec::new();
                if node == self.root() && self.arm().is_some() {
                    operations.push(operation(PatternDecision::EnterAttempt {
                        scope: index.nodes()[node.0 as usize].scope,
                    }));
                }
                operations.push(operation(
                    if matches!(
                        row.kind,
                        DecisionKind::Scalar(_) | DecisionKind::Tuple(_) | DecisionKind::Binding(_)
                    ) {
                        PatternDecision::StartSource
                    } else {
                        PatternDecision::Enter
                    },
                ));
                emit(operations)
            }
            Phase::Operand(0) => emit(vec![operation(PatternDecision::SourceStep)]),
            Phase::Operand(1) => match &row.kind {
                DecisionKind::Scalar(scalar) => {
                    let comparison = scalar.first(parsed, body, key)?;
                    let upper_pending = comparison.upper_pending;
                    let place = self.source_place(node)?;
                    let mut operations = vec![
                        operation(PatternDecision::Observe),
                        Op::ReadPlace {
                            place,
                            destination: source,
                        },
                        Op::NumericHint {
                            parent: test,
                            child: expected,
                            left: Some(source),
                            operator: Some(comparison.operator),
                        },
                    ];
                    operations.extend(comparison.operations(
                        source,
                        expected,
                        test,
                        &HintRecipe::None,
                    ));
                    operations.push(operation(PatternDecision::Test {
                        result: test,
                        first: true,
                        upper_pending,
                    }));
                    emit(operations)
                }
                DecisionKind::Tuple(children) => emit(vec![
                    operation(PatternDecision::Observe),
                    operation(PatternDecision::CheckTuple {
                        place: self.source_place(node)?,
                        arity: u32::try_from(children.len()).map_err(|_| limit())?,
                    }),
                    operation(PatternDecision::Enter),
                ]),
                DecisionKind::Binding(_) => {
                    refuse("structural binding requires cached occurrence namespace admission")
                }
                _ => Err(invalid("non-source pattern entered a source result phase")),
            },
            Phase::Operand(2) => {
                let DecisionKind::Scalar(scalar) = &row.kind else {
                    return Err(invalid(
                        "non-scalar pattern entered an upper endpoint phase",
                    ));
                };
                if row.scalar_upper_pending() != Some(true) {
                    return Err(invalid("pattern has no retained lower/upper continuation"));
                }
                let comparison = scalar.upper(parsed, body, key)?;
                let mut operations = vec![
                    operation(PatternDecision::Observe),
                    Op::Discard { source: test },
                    Op::NumericHint {
                        parent: test,
                        child: expected,
                        left: Some(source),
                        operator: Some(comparison.operator),
                    },
                ];
                operations.extend(comparison.operations(source, expected, test, &HintRecipe::None));
                operations.push(operation(PatternDecision::Test {
                    result: test,
                    first: false,
                    upper_pending: false,
                }));
                emit(operations)
            }
            Phase::Commit => {
                let mut operations = Vec::new();
                if matches!(row.kind, DecisionKind::Scalar(_)) {
                    operations.push(Op::Discard { source: test });
                    operations.push(Op::Discard { source });
                } else if matches!(row.kind, DecisionKind::Binding(_)) {
                    return refuse(
                        "structural binding requires cached occurrence namespace admission",
                    );
                }
                operations.push(operation(PatternDecision::Success));
                emit(operations)
            }
            Phase::Exit if matches!(row.kind, DecisionKind::Scalar(_)) => emit(vec![
                Op::Discard { source: test },
                Op::Discard { source },
                operation(PatternDecision::Fail { node: Some(node) }),
            ]),
            Phase::PatternCleanup if node == self.root() => {
                emit(vec![operation(PatternDecision::Cleanup)])
            }
            _ => Err(invalid("pattern source decision phase differs")),
        }
    }
    fn source_place(&self, node: NodeId) -> Result<SlotId, Diagnostic> {
        let row = self
            .row(node)
            .ok_or_else(|| invalid("pattern source row absent"))?;
        let scratch = if row.source_path.len() % 2 == 0 {
            node
        } else {
            self.owner()
        };
        self.index().nodes()[scratch.0 as usize].runtime_temporaries[0]
            .and_then(|value| value.0.checked_add(1))
            .map(SlotId)
            .ok_or_else(|| invalid("pattern source place scratch absent"))
    }
}
