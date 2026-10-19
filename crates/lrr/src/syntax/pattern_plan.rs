//! One immutable ordinary-source plan per pattern owner: a `match`
//! expression, a matched `let` statement (destructuring, `@` or `let`-`else`)
//! or an `if let`/`while let` clause. All owners lower to the same shared
//! transaction steps (`Open`, `Test`, `Expose`, `Conceal`, `Commit`, `Close`)
//! and the single VM matcher; generated syntax from macro expansions uses the
//! exact same route. Query dependencies establish currentness; installed
//! operations never call back here.
use super::{
    annotation_hint, draft, error, id_for, intrinsic_place_receiver, next_phase_operations,
    pattern_tree::{LocalItems, PatternTree},
    return_pc, statement_cfg,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    ir::{
        op::{HintRecipe, Op, PatternOperation, PrimitiveType},
        pc::{NodeId, PcKey, Phase, SlotId},
        recipe::{RecipeExit, VerifiedRecipe},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasLoopBody},
};
use std::sync::Arc;

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

/// A written root pattern and its retained tree. A refused tree faults only
/// when its owner actually reaches it.
#[derive(Debug)]
struct Root {
    node: NodeId,
    tree: Result<Arc<PatternTree>, Diagnostic>,
}
#[derive(Debug)]
struct Arm {
    id: NodeId,
    pattern: Root,
    guard: Option<NodeId>,
    result: NodeId,
    attributed: bool,
}
#[derive(Debug)]
struct MatchPlan {
    id: NodeId,
    scrutinee: NodeId,
    place: bool,
    hint: HintRecipe,
    arms: Vec<Arm>,
    next: PcKey,
}
#[derive(Debug)]
struct LetPlan {
    id: NodeId,
    initializer: NodeId,
    place: bool,
    hint: HintRecipe,
    pattern: Root,
    otherwise: Option<NodeId>,
}
#[derive(Debug)]
struct ConditionPlan {
    id: NodeId,
    scrutinee: NodeId,
    place: bool,
    hint: HintRecipe,
    pattern: Root,
    next: PcKey,
}
/// `for PAT in HEAD BODY`: rustc's `IntoIterator::into_iter(HEAD)` once,
/// then per iteration `Iterator::next(&mut iter)` matched against `Some(PAT)`
/// through the same transaction as every other pattern owner.
#[derive(Debug)]
struct ForPlan {
    id: NodeId,
    head: NodeId,
    pattern: Root,
    body: NodeId,
    next: PcKey,
}
/// A call frame's parameters: each binds from its staged call argument, a
/// plain closure identifier directly and every other pattern (`|&x|`,
/// `|(a, b)|`, and a function's `fn f((a, b): T)`) through the shared
/// transaction with the closure (or the function's parameter list) as owner.
/// A function's plain parameters are not part of its plan: they stay frame
/// parameters read at each use.
#[derive(Debug)]
struct ClosurePlan {
    id: NodeId,
    parameters: Vec<ClosureParameterPlan>,
    /// Where execution continues once every parameter is bound.
    after: PcKey,
    /// The closure scope entered when the closure's call frame starts (its
    /// Commit phase); a function's parameter scope is entered by its root
    /// block together with the argument check (its owner starts at Entry).
    scope: Option<crate::syntax::body_index::ScopeId>,
}
#[derive(Debug)]
enum ClosureParameterPlan {
    Binding {
        binding: NodeId,
        ordinal: u32,
        staging: SlotId,
        hint: HintRecipe,
        writable: bool,
    },
    Pattern {
        pattern: Root,
        ordinal: u32,
        staging: SlotId,
    },
}
impl ClosurePlan {
    fn pattern_roots(&self) -> Vec<&Root> {
        self.parameters
            .iter()
            .filter_map(|parameter| match parameter {
                ClosureParameterPlan::Pattern { pattern, .. } => Some(pattern),
                ClosureParameterPlan::Binding { .. } => None,
            })
            .collect()
    }
}
#[derive(Debug)]
enum Prepared {
    Match(MatchPlan),
    Let(LetPlan),
    Condition(ConditionPlan),
    For(ForPlan),
    Closure(ClosurePlan),
    Refused(Diagnostic),
    Disabled(Box<VerifiedRecipe>),
}

#[derive(Debug)]
pub struct PatternSourcePlan {
    owner: PcKey,
    parsed: Arc<ParsedFile>,
    index: Arc<BodyIndex>,
    body: BodyHandle,
    prepared: Prepared,
    fingerprint: Digest,
    bytes: usize,
}
impl PatternSourcePlan {
    /// Capturing a place used as a scrutinee needs these same retained trees,
    /// never executable preparation of a guard, arm result or else block.
    pub(crate) fn scrutinee(&self) -> Option<NodeId> {
        match &self.prepared {
            Prepared::Match(plan) => Some(plan.scrutinee),
            Prepared::Let(plan) => Some(plan.initializer),
            Prepared::Condition(plan) => Some(plan.scrutinee),
            _ => None,
        }
    }
    pub(crate) fn owner(&self) -> &PcKey {
        &self.owner
    }
    pub(crate) fn parsed(&self) -> &Arc<ParsedFile> {
        &self.parsed
    }
    pub(crate) fn index(&self) -> &Arc<BodyIndex> {
        &self.index
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    fn roots(&self) -> Vec<&Root> {
        match &self.prepared {
            Prepared::Match(plan) => plan.arms.iter().map(|arm| &arm.pattern).collect(),
            Prepared::Let(plan) => vec![&plan.pattern],
            Prepared::Condition(plan) => vec![&plan.pattern],
            Prepared::For(plan) => vec![&plan.pattern],
            Prepared::Closure(plan) => plan.pattern_roots(),
            Prepared::Refused(_) | Prepared::Disabled(_) => Vec::new(),
        }
    }
    /// Retained products for ownership observation; refused roots own no tree.
    pub(crate) fn trees(&self) -> impl Iterator<Item = &Arc<PatternTree>> {
        self.roots().into_iter().filter_map(|root| root.tree.as_ref().ok())
    }
    /// Semantic consumers must retain a refused root rather than treating it
    /// as an empty, inspection-only pattern.
    pub(crate) fn binding_trees(&self) -> impl Iterator<Item = Result<&Arc<PatternTree>, &Diagnostic>> {
        self.roots().into_iter().map(|root| root.tree.as_ref())
    }

    /// Indexed ownership only. An expression inside a match result does not
    /// become a pattern phase merely because it has a match ancestor; only the
    /// owner, its arms and its written root patterns have owner phases.
    pub(crate) fn owner_for(index: &BodyIndex, key: &PcKey) -> Result<Option<PcKey>, Diagnostic> {
        if &key.body != index.body() {
            return Err(error("pattern phase body differs"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("pattern phase node absent"))?;
        let owner = |node: &crate::syntax::body_index::StructuralNode| match node.role {
            NodeRole::Expression(ExpressionKind::Match | ExpressionKind::For) => true,
            NodeRole::Expression(ExpressionKind::Other) => node.clause_scope.is_some(),
            NodeRole::Statement => node.pattern_statement,
            // The closure's own call frame starts at its Commit phase; its
            // creation (Entry) and return (Operand) are ordinary closure phases.
            NodeRole::Expression(ExpressionKind::Closure) => node.pattern_statement,
            // A function's parameter list owns its pattern parameters.
            NodeRole::Syntax => node.pattern_statement,
            _ => false,
        };
        if owner(node) {
            if node.role == NodeRole::Expression(ExpressionKind::Closure)
                && Phase::from_code(key.phase) != Phase::Commit
            {
                return Ok(None);
            }
            return index.pc(node.id, Phase::Entry).map(Some);
        }
        let parent = |node: &crate::syntax::body_index::StructuralNode| {
            node.parent
                .and_then(|parent| index.nodes().get(parent.0 as usize))
        };
        match node.role {
            NodeRole::MatchArm => {
                // MatchArmList is an indexed syntax wrapper between arm and match.
                let mut current = parent(node);
                while let Some(ancestor) = current {
                    if ancestor.role == NodeRole::Expression(ExpressionKind::Match) {
                        return index.pc(ancestor.id, Phase::Entry).map(Some);
                    }
                    if ancestor.role != NodeRole::Syntax {
                        return Ok(None);
                    }
                    current = parent(ancestor);
                }
                Ok(None)
            }
            NodeRole::Pattern => {
                let Some(direct) = parent(node) else {
                    return Ok(None);
                };
                if direct.role == NodeRole::MatchArm {
                    return Self::owner_for(index, &index.pc(direct.id, Phase::Entry)?);
                }
                if owner(direct)
                    && matches!(
                        direct.role,
                        NodeRole::Statement | NodeRole::Expression(ExpressionKind::For)
                    )
                {
                    return index.pc(direct.id, Phase::Entry).map(Some);
                }
                // A closure parameter's root pattern: pattern, `Param`, `ParamList`.
                if direct.role == NodeRole::Syntax
                    && let Some(list) = parent(direct)
                    && list.role == NodeRole::Syntax
                    && let Some(closure) = parent(list)
                    && closure.role == NodeRole::Expression(ExpressionKind::Closure)
                    && closure.pattern_statement
                {
                    return index.pc(closure.id, Phase::Entry).map(Some);
                }
                // A function parameter's root pattern: pattern, `Param`, the
                // parameter list that owns it.
                if direct.role == NodeRole::Syntax
                    && let Some(list) = parent(direct)
                    && list.role == NodeRole::Syntax
                    && list.pattern_statement
                {
                    return index.pc(list.id, Phase::Entry).map(Some);
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn lower(&self, key: &PcKey) -> Result<VerifiedRecipe, Diagnostic> {
        if Self::owner_for(&self.index, key)?.as_ref() != Some(&self.owner) {
            return Err(error("retained pattern phase belongs to another owner"));
        }
        Lowering {
            parsed: &self.parsed,
            body: &self.body,
            index: &self.index,
            let_hint: None,
        }
        .lower(&self.owner, &self.prepared, key)
    }
    /// A destructuring `let` whose written annotation the entered Type
    /// service selected (`let [a, b]: [u8; 2] = ..`, `let &x: &u8 = ..`):
    /// its initializer takes that selected Type as its hint.
    pub(crate) fn lower_let_annotated(
        &self,
        key: &PcKey,
        hint: HintRecipe,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        if Self::owner_for(&self.index, key)?.as_ref() != Some(&self.owner) {
            return Err(error("retained pattern phase belongs to another owner"));
        }
        Lowering {
            parsed: &self.parsed,
            body: &self.body,
            index: &self.index,
            let_hint: Some(&hint),
        }
        .lower(&self.owner, &self.prepared, key)
    }
    /// Whether this owner is a destructuring `let` evaluating its initializer
    /// as a value (an annotation's selected Type can hint it).
    pub(crate) fn let_value_owner(&self) -> bool {
        matches!(&self.prepared, Prepared::Let(plan) if !plan.place)
    }
}

/// Phase lowering over one owner's prepared plan. Shared by the memoized
/// query route and direct pure lowering; both select identical recipes.
struct Lowering<'a> {
    parsed: &'a ParsedFile,
    body: &'a BodyHandle,
    index: &'a BodyIndex,
    /// A destructuring `let` annotation's entered-Type selection, which
    /// replaces the plan's syntactic initializer hint.
    let_hint: Option<&'a HintRecipe>,
}
impl Lowering<'_> {
    fn lower(
        &self,
        owner: &PcKey,
        prepared: &Prepared,
        key: &PcKey,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        self.index
            .pc(NodeId(key.node), Phase::from_code(key.phase))?;
        let result = match prepared {
            Prepared::Match(plan) => self.lower_match(plan, key),
            Prepared::Let(plan) => self.lower_let(plan, key),
            Prepared::Condition(plan) => self.lower_condition(plan, key),
            Prepared::For(plan) => self.lower_for(plan, key),
            Prepared::Closure(plan) => self.lower_closure(plan, key),
            Prepared::Disabled(recipe) if key == owner => Ok(recipe.as_ref().clone()),
            Prepared::Disabled(_) => Err(unsupported(
                "disabled pattern owner has no evaluated continuation",
            )),
            Prepared::Refused(diagnostic) => Err(diagnostic.clone()),
        };
        match result {
            Err(error) if error.code == "LRR-E-UNSUPPORTED" => self.refuse(key, &error.message),
            other => other,
        }
    }
    fn refuse(&self, key: &PcKey, message: &str) -> Result<VerifiedRecipe, Diagnostic> {
        crate::frontend::thunks::unsupported(
            self.index,
            key.clone(),
            message.into(),
            self.index.pc(NodeId(key.node), Phase::Exit)?,
        )
    }
    fn value(&self, node: NodeId) -> Result<SlotId, Diagnostic> {
        self.index.nodes()[node.0 as usize]
            .value
            .ok_or_else(|| error("pattern owner operand value absent"))
    }
    fn source(&self, node: NodeId, place: bool) -> Result<SlotId, Diagnostic> {
        let node = &self.index.nodes()[node.0 as usize];
        if place { node.place } else { node.value }
            .ok_or_else(|| error("pattern scrutinee slot absent"))
    }
    fn test_slot(&self, owner: NodeId) -> Result<SlotId, Diagnostic> {
        crate::ir::validate::pattern_owner_test_slot(self.index, owner)
            .ok_or_else(|| error("pattern owner Bool test slot absent"))
    }
    fn operand(&self, owner: NodeId, child: NodeId) -> Option<u16> {
        self.index.nodes()[owner.0 as usize]
            .operands
            .iter()
            .position(|operand| *operand == child)
            .and_then(|position| u16::try_from(position).ok())
    }
    fn tree(&self, root: &Root) -> Result<Arc<PatternTree>, Diagnostic> {
        root.tree.clone()
    }
    fn emit(
        &self,
        key: &PcKey,
        ops: Vec<Op>,
        exit: RecipeExit,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        draft(self.index, key, ops, exit)
    }
    fn lower_match(&self, plan: &MatchPlan, key: &PcKey) -> Result<VerifiedRecipe, Diagnostic> {
        let index = self.index;
        let owner = plan.id;
        let test = self.test_slot(owner)?;
        let destination = self.value(owner)?;
        let pattern = |operation| Op::Pattern { operation };
        let node = NodeId(key.node);
        let phase = Phase::from_code(key.phase);
        let first_arm = |from: usize| -> Result<PcKey, Diagnostic> {
            match plan.arms.get(from) {
                Some(arm) => index.pc(arm.id, Phase::Entry),
                None => index.pc(owner, Phase::Exit),
            }
        };
        let enter_result = |arm: &Arm, mut ops: Vec<Op>| -> Result<VerifiedRecipe, Diagnostic> {
            ops.push(pattern(PatternOperation::Commit { owner }));
            ops.push(Op::NumericHint {
                parent: destination,
                child: self.value(arm.result)?,
                left: None,
                operator: None,
            });
            self.emit(
                key,
                ops,
                RecipeExit::Next(index.pc(arm.result, Phase::Entry)?),
            )
        };
        let test_arm =
            |arm: &Arm, ops: &mut Vec<Op>, retry: bool| -> Result<RecipeExit, Diagnostic> {
                ops.push(pattern(PatternOperation::Test {
                    owner,
                    tree: self.tree(&arm.pattern)?,
                    result: test,
                    retry,
                }));
                Ok(RecipeExit::Branch {
                    condition: test,
                    yes: index.pc(arm.id, Phase::Commit)?,
                    no: index.pc(arm.id, Phase::Exit)?,
                })
            };
        if node == owner {
            return match phase {
                Phase::Entry => self.emit(
                    key,
                    if plan.place || plan.hint == HintRecipe::None {
                        vec![]
                    } else {
                        vec![Op::SetHint {
                            child: self.value(plan.scrutinee)?,
                            hint: plan.hint.clone(),
                        }]
                    },
                    RecipeExit::Next(index.pc(plan.scrutinee, Phase::Entry)?),
                ),
                Phase::Exit => self.emit(key, vec![], RecipeExit::MatchNoArm),
                Phase::Operand(n) => {
                    let child = *index.nodes()[owner.0 as usize]
                        .operands
                        .get(usize::from(n))
                        .ok_or_else(|| error("match operand phase absent"))?;
                    if child == plan.scrutinee {
                        return self.emit(
                            key,
                            vec![pattern(PatternOperation::Open {
                                owner,
                                source: self.source(plan.scrutinee, plan.place)?,
                            })],
                            RecipeExit::Next(first_arm(0)?),
                        );
                    }
                    if let Some(arm) = plan.arms.iter().find(|arm| arm.guard == Some(child)) {
                        let guard_scope = index.nodes()[arm.id.0 as usize]
                            .guard_scope
                            .ok_or_else(|| error("indexed match guard scope absent"))?;
                        return self.emit(
                            key,
                            vec![Op::ScopeLeave { scope: guard_scope }],
                            RecipeExit::Branch {
                                condition: self.value(child)?,
                                yes: index.pc(arm.pattern.node, Phase::Commit)?,
                                no: index.pc(arm.pattern.node, Phase::Exit)?,
                            },
                        );
                    }
                    if let Some(arm) = plan.arms.iter().find(|arm| arm.result == child) {
                        return self.emit(
                            key,
                            vec![
                                Op::ResultTransfer {
                                    source: self.value(arm.result)?,
                                    destination,
                                },
                                Op::ScopeLeave {
                                    scope: index.nodes()[arm.id.0 as usize].scope,
                                },
                                pattern(PatternOperation::Close { owner }),
                            ],
                            RecipeExit::Next(plan.next.clone()),
                        );
                    }
                    Err(unsupported("match operand has no ordinary continuation"))
                }
                _ => Err(unsupported("invalid match phase")),
            };
        }
        if let Some(position) = plan.arms.iter().position(|arm| arm.id == node) {
            let arm = &plan.arms[position];
            if arm.attributed {
                return Err(unsupported(
                    "reached match arm attributes require macro integration",
                ));
            }
            let arm_scope = index.nodes()[arm.id.0 as usize].scope;
            return match phase {
                Phase::Entry => {
                    let mut ops = Vec::new();
                    let exit = test_arm(arm, &mut ops, false)?;
                    self.emit(key, ops, exit)
                }
                Phase::Commit => {
                    let ops = vec![
                        Op::Discard { source: test },
                        Op::ScopeEnter { scope: arm_scope },
                    ];
                    match arm.guard {
                        None => enter_result(arm, ops),
                        Some(guard) => {
                            let mut ops = ops;
                            ops.push(pattern(PatternOperation::Expose { owner }));
                            ops.push(Op::SetHint {
                                child: self.value(guard)?,
                                hint: HintRecipe::Primitive(PrimitiveType::Bool),
                            });
                            self.emit(key, ops, RecipeExit::Next(index.pc(guard, Phase::Entry)?))
                        }
                    }
                }
                Phase::Exit => self.emit(
                    key,
                    vec![Op::Discard { source: test }],
                    RecipeExit::Next(first_arm(position + 1)?),
                ),
                _ => Err(unsupported("invalid match arm phase")),
            };
        }
        if let Some(arm) = plan.arms.iter().find(|arm| arm.pattern.node == node) {
            let Some(guard) = arm.guard else {
                return Err(unsupported(
                    "unguarded arm pattern has no guard continuation",
                ));
            };
            let guard_value = self.value(guard)?;
            return match phase {
                Phase::Commit => enter_result(
                    arm,
                    vec![
                        Op::Discard {
                            source: guard_value,
                        },
                        pattern(PatternOperation::Conceal { owner }),
                    ],
                ),
                Phase::Exit => {
                    let mut ops = vec![
                        Op::Discard {
                            source: guard_value,
                        },
                        pattern(PatternOperation::Conceal { owner }),
                        Op::ScopeLeave {
                            scope: index.nodes()[arm.id.0 as usize].scope,
                        },
                    ];
                    let exit = test_arm(arm, &mut ops, true)?;
                    self.emit(key, ops, exit)
                }
                _ => Err(unsupported("invalid guarded pattern phase")),
            };
        }
        Err(unsupported(
            "match pattern phase is outside its selected source",
        ))
    }
    fn lower_let(&self, plan: &LetPlan, key: &PcKey) -> Result<VerifiedRecipe, Diagnostic> {
        let index = self.index;
        let owner = plan.id;
        let test = self.test_slot(owner)?;
        let pattern = |operation| Op::Pattern { operation };
        let node = NodeId(key.node);
        let phase = Phase::from_code(key.phase);
        let statement = self
            .parsed
            .entered_node_syntax(self.body, index, owner)?
            .ok_or_else(|| error("let statement syntax attachment absent"))?;
        if node == owner {
            return match phase {
                Phase::Entry => {
                    let ops = if plan.place {
                        vec![]
                    } else {
                        vec![Op::SetHint {
                            child: self.value(plan.initializer)?,
                            hint: self.let_hint.unwrap_or(&plan.hint).clone(),
                        }]
                    };
                    self.emit(
                        key,
                        ops,
                        RecipeExit::Next(index.pc(plan.initializer, Phase::Entry)?),
                    )
                }
                Phase::Commit => self.emit(
                    key,
                    vec![
                        pattern(PatternOperation::Open {
                            owner,
                            source: self.source(plan.initializer, plan.place)?,
                        }),
                        pattern(PatternOperation::Test {
                            owner,
                            tree: self.tree(&plan.pattern)?,
                            result: test,
                            retry: false,
                        }),
                    ],
                    RecipeExit::Branch {
                        condition: test,
                        yes: index.pc(plan.pattern.node, Phase::Commit)?,
                        no: index.pc(plan.pattern.node, Phase::Exit)?,
                    },
                ),
                _ => Err(unsupported("invalid let statement phase")),
            };
        }
        if node != plan.pattern.node {
            return Err(unsupported("let pattern phase is outside its root"));
        }
        match phase {
            Phase::Commit => {
                let (ops, next) = next_phase_operations(
                    index,
                    self.body,
                    &statement,
                    vec![
                        Op::Discard { source: test },
                        pattern(PatternOperation::Commit { owner }),
                        pattern(PatternOperation::Close { owner }),
                    ],
                )?;
                self.emit(key, ops, RecipeExit::Next(next))
            }
            Phase::Exit => {
                let mut ops = vec![
                    Op::Discard { source: test },
                    pattern(PatternOperation::Close { owner }),
                ];
                if let Some(scope) = index.nodes()[owner.0 as usize].statement_scope {
                    ops.push(Op::ScopeLeave { scope });
                }
                match plan.otherwise {
                    Some(otherwise) => self.emit(
                        key,
                        ops,
                        RecipeExit::Next(index.pc(otherwise, Phase::Entry)?),
                    ),
                    None => self.emit(
                        key,
                        ops,
                        RecipeExit::Fault(crate::ir::op::DiagnosticTemplate {
                            message: "irrefutable let pattern did not match its initializer".into(),
                        }),
                    ),
                }
            }
            _ => Err(unsupported("invalid let pattern phase")),
        }
    }
    fn lower_for(&self, plan: &ForPlan, key: &PcKey) -> Result<VerifiedRecipe, Diagnostic> {
        let index = self.index;
        let owner = plan.id;
        let test = self.test_slot(owner)?;
        let pattern = |operation| Op::Pattern { operation };
        let node = &index.nodes()[owner.0 as usize];
        let iteration = node
            .condition_scope
            .ok_or_else(|| error("for iteration scope absent"))?;
        let [Some(iterator), Some(next)] = node.runtime_temporaries else {
            return Err(error("for iterator temporaries absent"));
        };
        let destination = self.value(owner)?;
        let phase = Phase::from_code(key.phase);
        if NodeId(key.node) == plan.pattern.node {
            return match phase {
                // `Some(PAT) => BODY`: bind, then run one iteration's body.
                Phase::Commit => self.emit(
                    key,
                    vec![
                        Op::Discard { source: test },
                        pattern(PatternOperation::Commit { owner }),
                        pattern(PatternOperation::Close { owner }),
                    ],
                    RecipeExit::Next(index.pc(plan.body, Phase::Entry)?),
                ),
                // `None => break`: the iterator is dropped with the loop.
                Phase::Exit => self.emit(
                    key,
                    vec![
                        Op::Discard { source: test },
                        pattern(PatternOperation::Close { owner }),
                        Op::ScopeLeave { scope: iteration },
                        Op::ScopeLeave { scope: node.scope },
                        Op::Unit { destination },
                    ],
                    RecipeExit::Next(plan.next.clone()),
                ),
                _ => Err(unsupported("invalid for pattern phase")),
            };
        }
        if NodeId(key.node) != owner {
            return Err(unsupported("for pattern phase is outside its owner"));
        }
        match phase {
            Phase::Entry => self.emit(
                key,
                vec![Op::ScopeEnter { scope: node.scope }],
                RecipeExit::Next(index.pc(plan.head, Phase::Entry)?),
            ),
            Phase::Operand(n) if self.operand(owner, plan.head) == Some(n) => self.emit(
                key,
                vec![Op::IteratorCall {
                    call: crate::ir::op::IteratorCall::IntoIter,
                    source: self.value(plan.head)?,
                    destination: iterator,
                }],
                RecipeExit::Next(index.pc(owner, Phase::Commit)?),
            ),
            // The loop head: one `next` call and its `Some(PAT)` test.
            Phase::Commit => self.emit(
                key,
                vec![
                    Op::ScopeEnter { scope: iteration },
                    Op::IteratorCall {
                        call: crate::ir::op::IteratorCall::Next,
                        source: iterator,
                        destination: next,
                    },
                    pattern(PatternOperation::Open {
                        owner,
                        source: next,
                    }),
                    pattern(PatternOperation::Test {
                        owner,
                        tree: self.tree(&plan.pattern)?,
                        result: test,
                        retry: false,
                    }),
                ],
                RecipeExit::Branch {
                    condition: test,
                    yes: index.pc(plan.pattern.node, Phase::Commit)?,
                    no: index.pc(plan.pattern.node, Phase::Exit)?,
                },
            ),
            Phase::Operand(n) if self.operand(owner, plan.body) == Some(n) => self.emit(
                key,
                vec![
                    Op::CheckUnit {
                        source: self.value(plan.body)?,
                    },
                    Op::ScopeLeave { scope: iteration },
                ],
                RecipeExit::Backedge(index.pc(owner, Phase::Commit)?),
            ),
            // Resumed by `break` after its iteration was left.
            Phase::Exit => self.emit(
                key,
                vec![Op::ScopeLeave { scope: node.scope }],
                RecipeExit::Next(plan.next.clone()),
            ),
            _ => Err(unsupported("invalid for phase")),
        }
    }
    /// Operations binding the call frame's parameters from `from` on, up to
    /// and including the next pattern parameter's `Open`/`Test`; that
    /// parameter's root pattern is returned. One `Test` per recipe keeps the
    /// reached pattern names prepared per installed recipe.
    fn closure_parameters_from<'p>(
        &self,
        plan: &'p ClosurePlan,
        from: usize,
        ops: &mut Vec<Op>,
    ) -> Result<Option<&'p Root>, Diagnostic> {
        let test = self.test_slot(plan.id)?;
        for parameter in plan.parameters.iter().skip(from) {
            let (staging, ordinal) = match parameter {
                ClosureParameterPlan::Binding {
                    staging, ordinal, ..
                }
                | ClosureParameterPlan::Pattern {
                    staging, ordinal, ..
                } => (*staging, *ordinal),
            };
            ops.push(Op::ReadParameter {
                ordinal,
                destination: staging,
            });
            match parameter {
                ClosureParameterPlan::Binding {
                    binding,
                    hint,
                    writable,
                    ..
                } => ops.push(Op::BindLocal {
                    binding: *binding,
                    source: staging,
                    hint: hint.clone(),
                    writable: *writable,
                }),
                ClosureParameterPlan::Pattern { pattern, .. } => {
                    ops.push(Op::Pattern {
                        operation: PatternOperation::Open {
                            owner: plan.id,
                            source: staging,
                        },
                    });
                    ops.push(Op::Pattern {
                        operation: PatternOperation::Test {
                            owner: plan.id,
                            tree: self.tree(pattern)?,
                            result: test,
                            retry: false,
                        },
                    });
                    return Ok(Some(pattern));
                }
            }
        }
        Ok(None)
    }
    fn closure_continuation(
        &self,
        plan: &ClosurePlan,
        reached: Option<&Root>,
    ) -> Result<RecipeExit, Diagnostic> {
        Ok(match reached {
            Some(root) => RecipeExit::Branch {
                condition: self.test_slot(plan.id)?,
                yes: self.index.pc(root.node, Phase::Commit)?,
                no: self.index.pc(root.node, Phase::Exit)?,
            },
            None => RecipeExit::Next(plan.after.clone()),
        })
    }
    /// The call frame's parameters: a closure enters its scope and binds the
    /// parameters in order; a function binds its pattern parameters in order
    /// inside the parameter scope its root block entered. Each pattern
    /// parameter commits before the next one is read.
    fn lower_closure(&self, plan: &ClosurePlan, key: &PcKey) -> Result<VerifiedRecipe, Diagnostic> {
        let owner = plan.id;
        let test = self.test_slot(owner)?;
        let pattern = |operation| Op::Pattern { operation };
        let node = NodeId(key.node);
        if node == owner {
            if Phase::from_code(key.phase)
                != if plan.scope.is_some() {
                    Phase::Commit
                } else {
                    Phase::Entry
                }
            {
                return Err(unsupported("invalid call frame parameter phase"));
            }
            let mut ops: Vec<Op> = plan
                .scope
                .map(|scope| Op::ScopeEnter { scope })
                .into_iter()
                .collect();
            let reached = self.closure_parameters_from(plan, 0, &mut ops)?;
            let exit = self.closure_continuation(plan, reached)?;
            return self.emit(key, ops, exit);
        }
        let Some(position) = plan.parameters.iter().position(|parameter| {
            matches!(parameter, ClosureParameterPlan::Pattern { pattern, .. } if pattern.node == node)
        }) else {
            return Err(unsupported("call frame pattern phase is outside its parameters"));
        };
        match Phase::from_code(key.phase) {
            Phase::Commit => {
                let mut ops = vec![
                    Op::Discard { source: test },
                    pattern(PatternOperation::Commit { owner }),
                    pattern(PatternOperation::Close { owner }),
                ];
                let reached = self.closure_parameters_from(plan, position + 1, &mut ops)?;
                let exit = self.closure_continuation(plan, reached)?;
                self.emit(key, ops, exit)
            }
            Phase::Exit => self.emit(
                key,
                vec![
                    Op::Discard { source: test },
                    pattern(PatternOperation::Close { owner }),
                ],
                RecipeExit::Fault(crate::ir::op::DiagnosticTemplate {
                    message: "irrefutable parameter pattern did not match its argument".into(),
                }),
            ),
            _ => Err(unsupported("invalid call frame parameter pattern phase")),
        }
    }
    fn lower_condition(
        &self,
        plan: &ConditionPlan,
        key: &PcKey,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let index = self.index;
        let owner = plan.id;
        let result = self.test_slot(owner)?;
        let pattern = |operation| Op::Pattern { operation };
        if NodeId(key.node) != owner {
            return Err(unsupported("condition pattern phase is outside its owner"));
        }
        let node = &index.nodes()[owner.0 as usize];
        match Phase::from_code(key.phase) {
            Phase::Entry => {
                let mut ops = vec![Op::ScopeEnter {
                    scope: node
                        .clause_scope
                        .ok_or_else(|| error("condition clause scope absent"))?,
                }];
                if !plan.place && plan.hint != HintRecipe::None {
                    ops.push(Op::SetHint {
                        child: self.value(plan.scrutinee)?,
                        hint: plan.hint.clone(),
                    });
                }
                self.emit(
                    key,
                    ops,
                    RecipeExit::Next(index.pc(plan.scrutinee, Phase::Entry)?),
                )
            }
            Phase::Operand(n) if self.operand(owner, plan.scrutinee) == Some(n) => self.emit(
                key,
                vec![
                    pattern(PatternOperation::Open {
                        owner,
                        source: self.source(plan.scrutinee, plan.place)?,
                    }),
                    pattern(PatternOperation::Test {
                        owner,
                        tree: self.tree(&plan.pattern)?,
                        result,
                        retry: false,
                    }),
                ],
                RecipeExit::Branch {
                    condition: result,
                    yes: index.pc(owner, Phase::Commit)?,
                    no: index.pc(owner, Phase::Exit)?,
                },
            ),
            Phase::Commit => self.emit(
                key,
                vec![
                    pattern(PatternOperation::Commit { owner }),
                    pattern(PatternOperation::Close { owner }),
                ],
                RecipeExit::Next(plan.next.clone()),
            ),
            Phase::Exit => self.emit(
                key,
                vec![pattern(PatternOperation::Close { owner })],
                RecipeExit::Next(plan.next.clone()),
            ),
            _ => Err(unsupported("invalid condition pattern phase")),
        }
    }
}

impl ParsedFile {
    fn pattern_root(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        local_items: &LocalItems,
        pattern: &ast::Pat,
    ) -> Result<Root, Diagnostic> {
        let node = id_for(index, body, pattern.syntax())?;
        let tree = match PatternTree::build(self, body, index, local_items, pattern) {
            Ok(tree) => Ok(Arc::new(tree)),
            Err(error) if error.code == "LRR-E-UNSUPPORTED" => Err(error),
            Err(error) => return Err(error),
        };
        Ok(Root { node, tree })
    }
    pub(crate) fn prepare_pattern_plan(
        self: &Arc<Self>,
        body: &BodyHandle,
        index: Arc<BodyIndex>,
        owner: &PcKey,
    ) -> Result<PatternSourcePlan, Diagnostic> {
        let prepared = self.prepared_pattern(body, &index, owner)?;
        Ok(PatternSourcePlan::new(owner, self, index, body, prepared))
    }
    /// Direct pure lowering of one pattern-owner phase. The memoized query
    /// route retains the same prepared plan instead of rebuilding it.
    pub(crate) fn initial_pattern_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<Option<VerifiedRecipe>, Diagnostic> {
        let Some(owner) = PatternSourcePlan::owner_for(index, key)? else {
            return Ok(None);
        };
        let prepared = self.prepared_pattern(body, index, &owner)?;
        Lowering {
            parsed: self,
            body,
            index,
            let_hint: None,
        }
        .lower(&owner, &prepared, key)
        .map(Some)
    }
    fn prepared_pattern(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        owner: &PcKey,
    ) -> Result<Prepared, Diagnostic> {
        if owner.phase != 0
            || body.key() != owner.body
            || index.body() != &owner.body
            || index.profile() != &self.profile
        {
            return Err(error("pattern plan owning source/body/profile differs"));
        }
        // A closure owns only its Commit (call-frame entry) phase.
        let probe = if index.nodes()[owner.node as usize].role
            == NodeRole::Expression(ExpressionKind::Closure)
        {
            index.pc(NodeId(owner.node), Phase::Commit)?
        } else {
            owner.clone()
        };
        let syntax = self.entered_node_syntax(body,index,NodeId(owner.node))?
            .ok_or_else(|| error("pattern owner syntax attachment absent"))?;
        if PatternSourcePlan::owner_for(index, &probe)?.as_ref() != Some(owner)
            && !ast::LetStmt::can_cast(syntax.kind()) {
            return Err(error("pattern plan key is not its exact owning entry"));
        }
        // Preserve the ordinary entry admission when this match expression is
        // itself an enclosing match's boolean guard.
        if index.nodes()[owner.node as usize]
            .boolean_scope
            .is_some_and(|scope| {
                index.scopes()[scope.0 as usize].kind
                    == crate::syntax::body_index::ScopeKind::MatchGuard
            })
        {
            self.require_boolean_match_guard(body, index, NodeId(owner.node))?;
        }
        let (enabled, attributes) = statement_cfg(&syntax, index.cfg())?;
        if !enabled {
            return Ok(Prepared::Disabled(Box::new(
                self.initial_thunk(body, index, owner)?,
            )));
        }
        if attributes {
            return Ok(Prepared::Refused(unsupported(
                "entered pattern owner attributes require macro integration",
            )));
        }
        let local_items = LocalItems::collect(self, body)?;
        Ok(
            if let Some(expression) = ast::MatchExpr::cast(syntax.clone()) {
                match self.match_plan(body, index, &local_items, &expression)? {
                    Ok(plan) => Prepared::Match(plan),
                    Err(refused) => Prepared::Refused(refused),
                }
            } else if let Some(expression) = ast::ClosureExpr::cast(syntax.clone()) {
                let (Some(list), Some(closure_body)) = (expression.param_list(), expression.body())
                else {
                    return Ok(Prepared::Refused(unsupported("closure syntax absent")));
                };
                let staging = &index.nodes()[owner.node as usize].runtime_scalar_slots;
                let mut parameters = Vec::new();
                for (position, parameter) in list.params().enumerate() {
                    let ordinal = u32::try_from(position)
                        .map_err(|_| error("closure parameter ordinal overflow"))?;
                    let (Some(pattern), Some(staging)) = (parameter.pat(), staging.get(position))
                    else {
                        return Ok(Prepared::Refused(unsupported(
                            "closure parameter pattern or staging slot absent",
                        )));
                    };
                    let staging = *staging;
                    parameters.push(
                        if crate::syntax::body_index::closure_pattern_parameter(&parameter) {
                            ClosureParameterPlan::Pattern {
                                pattern: self.pattern_root(body, index, &local_items, &pattern)?,
                                ordinal,
                                staging,
                            }
                        } else {
                            let ast::Pat::IdentPat(ident) = &pattern else {
                                return Err(error("closure binding parameter is not an identifier"));
                            };
                            ClosureParameterPlan::Binding {
                                binding: id_for(index, body, ident.syntax())?,
                                ordinal,
                                staging,
                                // A written primitive annotation types the
                                // argument; any other is not re-enforced.
                                hint: super::hint(parameter.ty()).unwrap_or(HintRecipe::None),
                                writable: ident.mut_token().is_some(),
                            }
                        },
                    );
                }
                Prepared::Closure(ClosurePlan {
                    id: NodeId(owner.node),
                    parameters,
                    after: index.pc(id_for(index, body, closure_body.syntax())?, Phase::Entry)?,
                    scope: index.nodes()[owner.node as usize].closure_scope,
                })
            } else if let Some(list) = ast::ParamList::cast(syntax.clone()) {
                // A pattern function's parameter list (E-R5-010). Every written
                // parameter binds in order from its argument, `let PAT = arg;`
                // for each, so drops follow rustc's reverse parameter order
                // across plain and pattern parameters alike.
                let staging = &index.nodes()[owner.node as usize].runtime_scalar_slots;
                let receiver = u32::from(list.self_param().is_some());
                let mut parameters = Vec::new();
                for (position, parameter) in list.params().enumerate() {
                    let (Some(pattern), Some(staging)) = (parameter.pat(), staging.get(position))
                    else {
                        return Ok(Prepared::Refused(unsupported(
                            "function parameter pattern or staging slot absent",
                        )));
                    };
                    let ordinal = u32::try_from(position)
                        .ok()
                        .and_then(|position| position.checked_add(receiver))
                        .ok_or_else(|| error("function parameter ordinal overflow"))?;
                    parameters.push(
                        if crate::syntax::fn_parameters::pattern_parameter(&parameter) {
                            ClosureParameterPlan::Pattern {
                                pattern: self.pattern_root(body, index, &local_items, &pattern)?,
                                ordinal,
                                staging: *staging,
                            }
                        } else {
                            // `x`, `mut x` or `_`: the argument is the local.
                            ClosureParameterPlan::Binding {
                                binding: id_for(index, body, pattern.syntax())?,
                                ordinal,
                                staging: *staging,
                                hint: HintRecipe::None,
                                writable: matches!(&pattern, ast::Pat::IdentPat(ident)
                                    if ident.mut_token().is_some()),
                            }
                        },
                    );
                }
                let Some(block) = list
                    .syntax()
                    .parent()
                    .and_then(ast::Fn::cast)
                    .and_then(|function| function.body())
                else {
                    return Ok(Prepared::Refused(unsupported("function body block absent")));
                };
                Prepared::Closure(ClosurePlan {
                    id: NodeId(owner.node),
                    parameters,
                    after: super::block_first_pc(index, body, &block)?,
                    scope: None,
                })
            } else if let Some(expression) = ast::ForExpr::cast(syntax.clone()) {
                let (Some(pattern), Some(head), Some(loop_body)) =
                    (expression.pat(), expression.iterable(), expression.loop_body())
                else {
                    return Ok(Prepared::Refused(unsupported("for loop syntax absent")));
                };
                let node = id_for(index, body, pattern.syntax())?;
                let tree = match PatternTree::build_for(self, body, index, &local_items, &pattern) {
                    Ok(tree) => Ok(Arc::new(tree)),
                    Err(error) if error.code == "LRR-E-UNSUPPORTED" => Err(error),
                    Err(error) => return Err(error),
                };
                Prepared::For(ForPlan {
                    id: NodeId(owner.node),
                    head: id_for(index, body, head.syntax())?,
                    pattern: Root { node, tree },
                    body: id_for(index, body, loop_body.syntax())?,
                    next: return_pc(index, body, expression.syntax())?,
                })
            } else if let Some(expression) = ast::LetExpr::cast(syntax.clone()) {
                let (Some(pattern), Some(scrutinee)) = (expression.pat(), expression.expr()) else {
                    return Ok(Prepared::Refused(unsupported(
                        "condition pattern syntax absent",
                    )));
                };
                Prepared::Condition(ConditionPlan {
                    id: NodeId(owner.node),
                    scrutinee: id_for(index, body, scrutinee.syntax())?,
                    place: intrinsic_place_receiver(&scrutinee),
                    hint: literal_scrutinee_hint(&scrutinee, [pattern.clone()].iter()),
                    pattern: self.pattern_root(body, index, &local_items, &pattern)?,
                    next: return_pc(index, body, expression.syntax())?,
                })
            } else if let Some(statement) = ast::LetStmt::cast(syntax.clone()) {
                let (Some(pattern), Some(initializer)) = (statement.pat(), statement.initializer())
                else {
                    return Ok(Prepared::Refused(unsupported(
                        "uninitialized destructuring declaration awaits deferred initialization",
                    )));
                };
                let place = intrinsic_place_receiver(&initializer);
                let hint = if place {
                    HintRecipe::None
                } else {
                    match annotation_hint(self, statement.ty(), owner.body.definition)? {
                        Some(HintRecipe::None) | None if statement.ty().is_none() => {
                            literal_scrutinee_hint(&initializer, [pattern.clone()].iter())
                        }
                        hint => hint.unwrap_or(HintRecipe::None),
                    }
                };
                let otherwise = statement
                    .let_else()
                    .map(|otherwise| {
                        otherwise
                            .block_expr()
                            .ok_or_else(|| unsupported("let-else block absent"))
                            .and_then(|block| id_for(index, body, block.syntax()))
                    })
                    .transpose();
                match otherwise {
                    Ok(otherwise) => Prepared::Let(LetPlan {
                        id: NodeId(owner.node),
                        initializer: id_for(index, body, initializer.syntax())?,
                        place,
                        hint,
                        pattern: self.pattern_root(body, index, &local_items, &pattern)?,
                        otherwise,
                    }),
                    Err(refused) => Prepared::Refused(refused),
                }
            } else {
                return Err(error("pattern owner syntax role differs"));
            },
        )
    }
    fn match_plan(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        local_items: &LocalItems,
        expression: &ast::MatchExpr,
    ) -> Result<Result<MatchPlan, Diagnostic>, Diagnostic> {
        let id = id_for(index, body, expression.syntax())?;
        let Some(scrutinee) = expression.expr() else {
            return Ok(Err(unsupported("match scrutinee absent")));
        };
        let Some(list) = expression.match_arm_list() else {
            return Ok(Err(unsupported("match arm list absent")));
        };
        let mut arms = Vec::new();
        let mut written = Vec::new();
        for arm in list.arms() {
            let (enabled, attributed) = statement_cfg(arm.syntax(), index.cfg())?;
            if !enabled {
                continue;
            }
            let (Some(pattern), Some(result)) = (arm.pat(), arm.expr()) else {
                return Ok(Err(unsupported("match arm pattern or result absent")));
            };
            let guard = match arm.guard() {
                None => None,
                Some(guard) => match guard.condition() {
                    Some(condition) => Some(id_for(index, body, condition.syntax())?),
                    None => return Ok(Err(unsupported("match guard condition absent"))),
                },
            };
            written.push(pattern.clone());
            arms.push(Arm {
                id: id_for(index, body, arm.syntax())?,
                pattern: self.pattern_root(body, index, local_items, &pattern)?,
                guard,
                result: id_for(index, body, result.syntax())?,
                attributed,
            });
        }
        Ok(Ok(MatchPlan {
            id,
            scrutinee: id_for(index, body, scrutinee.syntax())?,
            place: intrinsic_place_receiver(&scrutinee),
            hint: literal_scrutinee_hint(&scrutinee, written.iter()),
            arms,
            next: return_pc(index, body, expression.syntax())?,
        }))
    }
}
impl PatternSourcePlan {
    fn new(
        owner: &PcKey,
        parsed: &Arc<ParsedFile>,
        index: Arc<BodyIndex>,
        body: &BodyHandle,
        prepared: Prepared,
    ) -> Self {
        let mut encoder = StableEncoder::new("lrr/v3/ordinary-pattern-source-plan");
        encoder.record(owner);
        encoder.digest(parsed.content);
        encoder.record(&parsed.profile);
        encoder.digest(index.semantic_identity());
        let roots: Vec<&Root> = match &prepared {
            Prepared::Match(plan) => plan.arms.iter().map(|arm| &arm.pattern).collect(),
            Prepared::Let(plan) => vec![&plan.pattern],
            Prepared::Condition(plan) => vec![&plan.pattern],
            Prepared::For(plan) => vec![&plan.pattern],
            Prepared::Closure(plan) => plan.pattern_roots(),
            Prepared::Refused(_) | Prepared::Disabled(_) => Vec::new(),
        };
        let mut bytes = std::mem::size_of::<Self>();
        encoder.sequence(&roots, |e, root| {
            e.u32(root.node.0);
            match &root.tree {
                Ok(tree) => {
                    e.u8(0);
                    e.digest(tree.identity());
                }
                Err(error) => {
                    e.u8(1);
                    e.string(error.code);
                    e.string(&error.message);
                }
            }
        });
        bytes = bytes.saturating_add(roots.len().saturating_mul(std::mem::size_of::<Root>()));
        if let Prepared::Refused(error) = &prepared {
            encoder.string(error.code);
            encoder.string(&error.message);
        }
        Self {
            owner: owner.clone(),
            parsed: Arc::clone(parsed),
            index,
            body: body.clone(),
            prepared,
            fingerprint: encoder.finish(),
            bytes,
        }
    }
}

/// Ordinary source constraint for a scrutinee whose every result leaf is a
/// numeric literal (a literal, or match/if/block expressions ending in
/// literals): rustc unifies its type with the written pattern literals, so a
/// suffixed literal pattern fixes it; otherwise an unsuffixed literal takes its
/// own default (i32 or f64). Any other scrutinee keeps its ordinary type, so a
/// reference scrutinee still reaches default binding modes.
fn literal_scrutinee_hint<'a>(
    scrutinee: &ast::Expr,
    patterns: impl Iterator<Item = &'a ast::Pat>,
) -> HintRecipe {
    use ra_ap_syntax::{AstToken, SyntaxKind};
    #[derive(Default)]
    struct Leaves {
        floating: bool,
        unsuffixed: bool,
    }
    fn leaves(expression: &ast::Expr, out: &mut Leaves, depth: usize) -> bool {
        match expression {
            ast::Expr::ParenExpr(group) => group
                .expr()
                .is_some_and(|inner| leaves(&inner, out, depth + 1)),
            ast::Expr::PrefixExpr(prefix)
                if prefix.op_token().is_some_and(|token| token.text() == "-") =>
            {
                prefix
                    .expr()
                    .is_some_and(|inner| leaves(&inner, out, depth + 1))
            }
            ast::Expr::Literal(literal) => match literal.token().kind() {
                SyntaxKind::INT_NUMBER => {
                    let Some(number) = ast::IntNumber::cast(literal.token()) else {
                        return false;
                    };
                    match number.suffix() {
                        None => out.unsuffixed = true,
                        Some("f32" | "f64") => out.floating = true,
                        Some(_) => {}
                    }
                    true
                }
                SyntaxKind::FLOAT_NUMBER => {
                    out.floating = true;
                    if ast::FloatNumber::cast(literal.token())
                        .is_some_and(|number| number.suffix().is_none())
                    {
                        out.unsuffixed = true;
                    }
                    true
                }
                _ => false,
            },
            ast::Expr::BlockExpr(block) if block.modifier().is_none() => block
                .tail_expr()
                .is_some_and(|tail| leaves(&tail, out, depth + 1)),
            ast::Expr::IfExpr(branch) => {
                let then = branch
                    .then_branch()
                    .is_some_and(|block| leaves(&ast::Expr::BlockExpr(block), out, depth + 1));
                then && match branch.else_branch() {
                    Some(ast::ElseBranch::Block(block)) => {
                        leaves(&ast::Expr::BlockExpr(block), out, depth + 1)
                    }
                    Some(ast::ElseBranch::IfExpr(nested)) => {
                        leaves(&ast::Expr::IfExpr(nested), out, depth + 1)
                    }
                    None => false,
                }
            }
            ast::Expr::MatchExpr(nested) => nested.match_arm_list().is_some_and(|list| {
                let mut any = false;
                list.arms().all(|arm| {
                    any = true;
                    arm.expr()
                        .is_some_and(|result| leaves(&result, out, depth + 1))
                }) && any
            }),
            _ => false,
        }
    }
    let mut found = Leaves::default();
    if !leaves(scrutinee, &mut found, 0) {
        return HintRecipe::None;
    }
    fn written(pattern: &ast::Pat) -> Option<PrimitiveType> {
        use ra_ap_syntax::ast::RangeItem;
        match pattern {
            ast::Pat::ParenPat(group) => group.pat().as_ref().and_then(written),
            ast::Pat::OrPat(or) => or.pats().find_map(|pattern| written(&pattern)),
            ast::Pat::IdentPat(ident) => ident.pat().as_ref().and_then(written),
            ast::Pat::RangePat(range) => range
                .start()
                .as_ref()
                .and_then(written)
                .or_else(|| range.end().as_ref().and_then(written)),
            ast::Pat::LiteralPat(literal) => {
                let token = literal.literal()?.token();
                match token.kind() {
                    SyntaxKind::INT_NUMBER => {
                        suffix_primitive(ast::IntNumber::cast(token)?.suffix()?)
                    }
                    SyntaxKind::FLOAT_NUMBER => {
                        suffix_primitive(ast::FloatNumber::cast(token)?.suffix()?)
                    }
                    SyntaxKind::BYTE => Some(PrimitiveType::U8),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    let patterns: Vec<&ast::Pat> = patterns.into_iter().collect();
    // A pattern naming something (a constant `X`, a path, a binding) types the
    // literal through the ordinary constraint service, never this fallback.
    fn names(pattern: &ast::Pat) -> bool {
        use ra_ap_syntax::AstNode;
        pattern.syntax().descendants().any(|node| {
            ast::IdentPat::can_cast(node.kind()) || ast::PathPat::can_cast(node.kind())
        })
    }
    match patterns.iter().copied().find_map(written) {
        Some(suffix) => HintRecipe::Primitive(suffix),
        None if found.unsuffixed && patterns.iter().any(|pattern| names(pattern)) => {
            HintRecipe::None
        }
        None if found.unsuffixed => HintRecipe::Primitive(if found.floating {
            PrimitiveType::F64
        } else {
            PrimitiveType::I32
        }),
        None => HintRecipe::None,
    }
}

fn suffix_primitive(suffix: &str) -> Option<PrimitiveType> {
    Some(match suffix {
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "i128" => PrimitiveType::I128,
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        "u128" => PrimitiveType::U128,
        "usize" => PrimitiveType::Usize,
        "f32" => PrimitiveType::F32,
        "f64" => PrimitiveType::F64,
        _ => return None,
    })
}
