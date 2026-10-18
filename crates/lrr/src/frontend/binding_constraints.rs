//! Demand source constraints, never a Rust expression or runtime evaluator.
//! Names and declared types come exclusively from the common source services.
//!
//! A bare numeric literal initializer gives its `let` binding an integer or
//! float inference variable. The demand visits only the syntax connected to
//! that variable through the built-in Rust typing relations recorded in the
//! retained source facts (operators, `let`, blocks, `if`, calls, returns,
//! casts, discarded statements and format arguments). A concrete primitive
//! from a written annotation, suffix, declared parameter or declared result
//! selects the type; a closed component with no concrete type takes Rust's
//! fallback (`i32`, `f64`). A relation the common services cannot answer yet
//! stays an explicit Pending goal and never becomes a guessed primitive.
use super::source_parameter_goals::ParameterTypeGoal;
use super::source_return_goals::ReturnTypeGoal;
use crate::facade::functions::LibraryFunction;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    ir::{
        op::PrimitiveType,
        pc::{NodeId, PcKey},
    },
    syntax::{
        pattern_tree::{PatternLiteral, PatternNode, RangeEndpoint},
        source_constraints::{
            FactKind, PatternStep, SourceConstraintPlan, SourceOperator, TypeShape,
        },
    },
    traits::term::TyTerm,
};
use ra_ap_syntax::AstNode;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
/// rustc's default `recursion_limit`.
pub(super) const RECURSION_LIMIT: usize = 128;
/// Whether a sub-query refusal only leaves this relation unanswered (an
/// explicit unsupported route or an absent name): the member stays a missing
/// goal. Currentness, stale-handle, ambiguity and internal faults propagate.
pub(super) fn not_closed(error: &Diagnostic) -> bool {
    matches!(error.code, "LRR-E-UNSUPPORTED" | "LRR-E-NAME-MISSING")
}
/// A refusal of the entered Type goal for a written annotation leaf. That
/// service reports written shapes it does not answer yet (a qualified generic
/// base `m::W<u8>`, a trait object `dyn T`) as `LRR-E-TYPE-CONTEXT`, the
/// code its own internal faults also carry, so such a leaf stays open (held
/// until the service distinguishes the two).
fn entered_type_refusal(error: &Diagnostic) -> bool {
    not_closed(error) || error.code == "LRR-E-TYPE-CONTEXT"
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ConstraintDemand {
    Binding,
    ValueUse,
    ExpressionRelation,
    SelectedDeclaration,
    ModuleEnvironment,
    ParameterType,
    NumericFamily,
    /// Method selection is not yet a common source service.
    MethodSelection,
    /// A declared function result Type is not available as a primitive.
    ResultType,
    /// A written annotation or cast target Type is not available as a primitive.
    AnnotationType,
}
impl ConstraintDemand {
    fn tag(self) -> u8 {
        match self {
            Self::Binding => 0,
            Self::ValueUse => 1,
            Self::ExpressionRelation => 2,
            Self::SelectedDeclaration => 3,
            Self::ModuleEnvironment => 4,
            Self::ParameterType => 5,
            Self::NumericFamily => 6,
            Self::MethodSelection => 7,
            Self::ResultType => 8,
            Self::AnnotationType => 9,
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BindingConstraintState {
    NoNumericDemand,
    /// Some use sits in syntax this service does not record (for example an
    /// index operand or an unexpanded macro/closure body) and no concrete
    /// Type was found. The existing held route keeps that decision.
    Unconstrained,
    Resolved(PrimitiveType),
    /// An initializer's open type arguments were decided by later uses; the
    /// answer is the written Type of the whole initializer.
    ResolvedType,
    Pending {
        node: NodeId,
        demand: ConstraintDemand,
    },
}
/// A private-minted source result, not a binding commit or execution grant.
#[derive(Debug, Clone)]
pub struct PreparedBindingConstraints {
    key: PcKey,
    structure: Digest,
    binding: Option<Digest>,
    // Indexed patterns of every local in the resolved component; they all
    // have the one resolved Type.
    members: Vec<NodeId>,
    state: BindingConstraintState,
    // The written initializer Type of `ResolvedType`.
    written: Option<Arc<crate::ir::written_type::WrittenType>>,
    // The callee's own generic parameters the same demand decided, stated as
    // a written turbofish would (`ResolvedType` of a source function call).
    arguments: Option<Arc<crate::ir::recipe::InferredArguments>>,
    related_numeric: Option<Arc<crate::ir::op::RelatedNumericPlan>>,
}
impl PreparedBindingConstraints {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn state(&self) -> BindingConstraintState {
        self.state
    }
    /// The written Type of an open-argument expression's `ResolvedType`.
    pub(crate) fn written(&self) -> Option<&crate::ir::written_type::WrittenType> {
        self.written.as_deref()
    }
    /// The callee generic parameters an open call's demand decided.
    pub(crate) fn inferred_arguments(&self) -> Option<&Arc<crate::ir::recipe::InferredArguments>> {
        self.arguments.as_ref()
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
    /// Phase selection from the retained source goal. The provider must attach
    /// its actual prepared-input guard before installing the resulting recipe.
    pub(crate) fn hint_for(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        key: &PcKey,
    ) -> Result<Option<crate::ir::op::HintRecipe>, Diagnostic> {
        if self.key.body != key.body
            || self.key.node != key.node
            || self.key.phase != 0
            || index.body() != &key.body
            || index.semantic_identity() != self.structure
        {
            return Err(fault("binding constraint source/body/node differs"));
        }
        match self.state {
            BindingConstraintState::NoNumericDemand | BindingConstraintState::Unconstrained => {
                Ok(None)
            }
            BindingConstraintState::Resolved(primitive) => {
                Ok(Some(crate::ir::op::HintRecipe::Primitive(primitive)))
            }
            BindingConstraintState::ResolvedType => Ok(self
                .written
                .as_ref()
                .map(|written| crate::ir::op::HintRecipe::Written(Arc::clone(written)))),
            BindingConstraintState::Pending { .. } => Err(fault(
                "numeric source constraint requires a missing common source goal",
            )),
        }
    }
    fn require_source(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
    ) -> Result<(), Diagnostic> {
        if self.key.phase != 0
            || index.body() != &self.key.body
            || index.semantic_identity() != self.structure
        {
            return Err(fault("binding constraint source/body differs"));
        }
        Ok(())
    }
    /// The resolved Type of an actual local `binding` (its indexed pattern)
    /// that belongs to this goal's component, for another consumer of that
    /// local such as a compound assignment. `None` for any other binding.
    pub(crate) fn local_primitive(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        binding: NodeId,
    ) -> Result<Option<PrimitiveType>, Diagnostic> {
        self.require_source(index)?;
        if self.members.binary_search(&binding).is_err() {
            return Ok(None);
        }
        Ok(match self.state {
            BindingConstraintState::Resolved(primitive) => Some(primitive),
            _ => None,
        })
    }
    /// A bare literal goal's resolved Type, for that literal's own lowering.
    pub(crate) fn literal_primitive(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        key: &PcKey,
    ) -> Result<Option<PrimitiveType>, Diagnostic> {
        self.require_source(index)?;
        if self.key.body != key.body || self.key.node != key.node || self.binding.is_some() {
            return Err(fault("literal constraint source node differs"));
        }
        Ok(match self.state {
            BindingConstraintState::Resolved(primitive) => Some(primitive),
            _ => None,
        })
    }
    pub(crate) fn has_related_numeric(&self) -> bool {
        self.related_numeric.is_some()
    }
    pub(crate) fn literal_hint(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        key: &PcKey,
    ) -> Result<Option<crate::ir::op::HintRecipe>, Diagnostic> {
        if let Some(primitive) = self.literal_primitive(index, key)? {
            return Ok(Some(crate::ir::op::HintRecipe::Primitive(primitive)));
        }
        Ok(self
            .related_numeric
            .as_ref()
            .map(|plan| crate::ir::op::HintRecipe::RelatedNumeric(Arc::clone(plan))))
    }
    fn pending(mut self, node: NodeId, demand: ConstraintDemand) -> QueryPoll<Self> {
        self.state = BindingConstraintState::Pending { node, demand };
        QueryPoll::Ready(Ok(self))
    }
}
/// The local binding a path use selects. A use whose goal awaits only the
/// classification of the prior pattern (`BindingClassification(id)`) has
/// selected that binding `id`: the pattern's own classification is answered
/// by `patterns::unclassified_binding`.
fn selected_local(goal: &crate::frontend::source_value_goals::EnteredValueGoal) -> Option<Digest> {
    use crate::frontend::source_value_goals::ValueGoalDemand;
    match goal.pending() {
        None => goal.local_binding(),
        Some(ValueGoalDemand::BindingClassification(id)) => Some(id),
        Some(_) => None,
    }
}
/// The selection awaits a declaration the service cannot read, or names an
/// enum variant. A variant is a member of its enum and owns no declaration
/// inputs: the numeric services read it as the pending constructor selection
/// (`None`, `Some(..)`), whichever route selected it (prelude, import, glob).
fn selection_pending(goal: &crate::frontend::source_value_goals::EnteredValueGoal) -> bool {
    goal.pending().is_some()
        || goal
            .declaration()
            .is_some_and(|declaration| declaration.variant().is_some())
}
/// A declaration owned by a body (its retained goal memos need no module
/// environment).
fn is_local(db: &mut Database, definition: Digest) -> bool {
    db.input(&crate::db::dependency::InputKey::LocalDeclarationOwner(
        definition,
    ))
    .is_some()
}
/// How many levels deep a declared Type nests.
pub(super) fn term_depth(term: &TyTerm) -> usize {
    use crate::traits::term::GenericArg;
    1 + match term {
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            term_depth(inner)
        }
        TyTerm::Tuple(members) => members.iter().map(term_depth).max().unwrap_or(0),
        TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(inner) => term_depth(inner),
                _ => 0,
            })
            .max()
            .unwrap_or(0),
        TyTerm::Projection(key) => term_depth(&key.trait_ref.self_ty),
        _ => 0,
    }
}
fn entry(key: &PcKey, node: NodeId) -> PcKey {
    PcKey {
        body: key.body.clone(),
        node: node.0,
        phase: 0,
    }
}
fn family(primitive: PrimitiveType) -> Option<bool> {
    match primitive {
        PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::I128
        | PrimitiveType::Isize
        | PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64
        | PrimitiveType::U128
        | PrimitiveType::Usize => Some(false),
        PrimitiveType::F32 | PrimitiveType::F64 => Some(true),
        _ => None,
    }
}
/// An unsuffixed literal reached through parentheses and unary `-`/`!`,
/// which rustc checks with its parent's expectation.
fn literal_operand(plan: &SourceConstraintPlan, mut node: NodeId) -> bool {
    loop {
        let Some(fact) = plan.fact(node) else {
            return false;
        };
        match (fact.kind(), fact.operation()) {
            (FactKind::Integer | FactKind::Float, _) => return true,
            (FactKind::Parenthesis, _)
            | (FactKind::Prefix, Some(SourceOperator::Negate | SourceOperator::Not)) => {
                match fact.operand(0).filter(|child| *child > node) {
                    Some(child) => node = child,
                    None => return false,
                }
            }
            _ => return false,
        }
    }
}
fn numeric_initializer(
    plan: &SourceConstraintPlan,
    mut node: NodeId,
) -> Result<Option<bool>, Diagnostic> {
    // Each step moves to a strictly later preorder child, so this terminates.
    loop {
        let fact = plan
            .fact(node)
            .ok_or_else(|| fault("numeric constraint initializer node absent"))?;
        match fact.kind() {
            FactKind::Integer => return Ok(Some(false)),
            FactKind::Float => return Ok(Some(true)),
            FactKind::Parenthesis => {
                let Some(child) = fact.operand(0).filter(|child| *child > node) else {
                    return Ok(None);
                };
                node = child;
            }
            FactKind::Prefix if fact.numeric_prefix() => {
                let Some(child) = fact.operand(0).filter(|child| *child > node) else {
                    return Ok(None);
                };
                node = child;
            }
            _ => return Ok(None),
        }
    }
}

/// One step of a structural projection from a value's Type, outermost first.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum Step {
    Deref,
    /// A tuple position, or a tuple struct's positional field.
    Field(u32),
    /// The element of an array or slice.
    Element,
    /// Strip every reference: a field access's base is auto-dereferenced
    /// before its field step (a pattern never takes this step).
    AutoDeref,
    /// A record struct's field, by its written name's member row.
    Named(u32),
    /// Field `field` (declaration order) of variant `variant` of enum
    /// `definition`.
    Variant {
        definition: Digest,
        variant: u32,
        field: u32,
    },
    /// `<T as IntoIterator>::Item` of the value's Type `T` (a `for` head).
    Item,
    /// The value's nominal Type's own type argument at this position.
    TypeArgument(u32),
}
#[path = "binding_constraint_bounds.rs"]
mod bounds;
#[path = "binding_constraint_callables.rs"]
mod callables;
#[path = "binding_constraint_comparison.rs"]
mod comparison;
#[path = "binding_constraint_deref.rs"]
mod deref;
#[path = "binding_constraint_generics.rs"]
mod generics;
#[path = "binding_constraint_iteration.rs"]
mod iteration;
#[path = "binding_constraint_lang.rs"]
mod lang;
#[path = "binding_constraint_methods.rs"]
mod methods;
#[path = "binding_constraint_modes.rs"]
mod modes;
#[path = "binding_constraint_nominal.rs"]
mod nominal;
#[path = "binding_constraint_observed.rs"]
mod observed;
#[path = "binding_constraint_open.rs"]
mod open;
#[path = "binding_constraint_patterns.rs"]
mod patterns;
#[path = "binding_constraint_projection.rs"]
mod projection;
use nominal::ConstructorSite;
/// How a member relates to the component's variable: projecting the
/// member's Type by `steps` yields the variable. A `strict` member was
/// reached only through scalar relations, whose unanswerable goals stay
/// explicitly pending; a member reached through a structural relation
/// (borrow, dereference, tuple, `match`, patterns) instead leaves an
/// unanswerable goal to the existing held route (Unconstrained).
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Via {
    steps: Vec<Step>,
    strict: bool,
}
impl Via {
    fn scalar() -> Self {
        Self {
            steps: Vec::new(),
            strict: true,
        }
    }
    fn is_scalar(&self) -> bool {
        self.steps.is_empty()
    }
    fn wrap(&self, step: Step) -> Self {
        let mut steps = Vec::with_capacity(self.steps.len() + 1);
        steps.push(step);
        steps.extend_from_slice(&self.steps);
        Self {
            steps,
            strict: false,
        }
    }
    fn unwrap(&self, step: Step) -> Option<Self> {
        (self.steps.first() == Some(&step)).then(|| Self {
            steps: self.steps[1..].to_vec(),
            strict: false,
        })
    }
    fn loose(&self) -> Self {
        Self {
            steps: self.steps.clone(),
            strict: false,
        }
    }
    /// `prefix` then these steps; strictness survives only an empty prefix.
    fn within(&self, prefix: &[Step]) -> Self {
        let mut steps = prefix.to_vec();
        steps.extend_from_slice(&self.steps);
        Self {
            steps,
            strict: self.strict && prefix.is_empty(),
        }
    }
}
/// A nominal Type's `position`th type argument.
fn type_argument(args: &[crate::traits::term::GenericArg], position: u32) -> Option<TyTerm> {
    args.iter()
        .filter_map(|arg| match arg {
            crate::traits::term::GenericArg::Type(term) => Some(term),
            _ => None,
        })
        .nth(position as usize)
        .cloned()
}
/// A written `_` Type hole (its initializer decides the Type).
fn hole() -> TyTerm {
    TyTerm::Infer(crate::traits::term::InferVarKey {
        transaction: Digest::default(),
        ordinal: u32::MAX,
    })
}
/// A call's ordinary selection.
enum Callee {
    Source(Digest),
    Facade(LibraryFunction),
    /// A local bound directly to a closure expression (its node): a closure
    /// has a unique type, so the local always calls that closure's body.
    Closure(NodeId),
    /// A parameter of the executing function whose Type states its
    /// `Fn*(..) -> R` signature (`binding_constraint_callables.rs`).
    Bound(Arc<callables::FnBound>),
}

/// One numeric inference variable's connected source component.
struct Component<'a> {
    key: &'a PcKey,
    plan: &'a SourceConstraintPlan,
    floating: bool,
    concrete: Option<(NodeId, PrimitiveType)>,
    pending: Option<(NodeId, ConstraintDemand)>,
    // A use in syntax that records no relation, or a possible unindexed use.
    unrecorded: bool,
    // The member being processed.
    via: Via,
    expressions: BTreeSet<(NodeId, Via)>,
    pattern_members: BTreeSet<(NodeId, Via)>,
    // Actual hygienic binding identity -> its indexed pattern.
    bindings: BTreeMap<(Digest, Via), NodeId>,
    work: Vec<(NodeId, Via)>,
    pattern_work: Vec<(NodeId, Via)>,
    patterns: Vec<(Digest, NodeId, Via)>,
    environment: Option<Arc<crate::expand::source_discovery::CompletedSourceEnvironment>>,
    /// The demand started from a literal (not a binding's declaration).
    literal_start: bool,
    constructors: BTreeMap<ConstructorSite, Option<Arc<nominal::Constructed>>>,
    /// Static expression Types answered for method selection and indexing.
    types: BTreeMap<NodeId, Option<TyTerm>>,
    /// The variable is an open type argument of a call result or fieldless
    /// constructor (`W::new()`, `None`), not a numeric literal's Type: any
    /// closed Type may decide it. See `binding_constraint_open.rs`.
    open: bool,
    /// Closed Types the relations state, by the node that states them.
    /// rustc checks in source order, so the earliest decides.
    candidates: BTreeMap<NodeId, TyTerm>,
    /// The family of an unsuffixed literal that reached an open variable
    /// (`false` integer, `true` float): rustc's fallback applies to it.
    literal_family: Option<bool>,
    /// An open variable's candidate Types that name another value's own type
    /// parameter (`Box<List<T>>` of a constructed `List { .. }`): the
    /// placeholder parameter, and the constructed expression and parameter
    /// ordinal whose Type decides it.
    dependents: BTreeMap<crate::traits::term::ParamKey, (NodeId, u32)>,
    /// Expressions whose open Types this demand is deciding for (it, or a
    /// demand nested inside one): none is asked again.
    deciding: Vec<NodeId>,
    /// The deepest declared Type this demand has read (see `admit`).
    declared_depth: usize,
    /// Declared parameter and result Types of selected functions, with the
    /// demands answering them deferred, read once per demand.
    goals: BTreeMap<(Digest, Option<u32>), (Option<TyTerm>, Vec<ConstraintDemand>)>,
    /// The demands `defer` is recording for the goal being read.
    deferred: Option<Vec<ConstraintDemand>>,
    /// The member projection at the root of the pattern being walked: the
    /// steps a nested pattern has consumed are the root's, less its own.
    root_via: Option<Via>,
    /// The `Fn*(..) -> R` bounds of selected functions' generic parameters,
    /// read once per demand.
    fn_bounds: BTreeMap<(Digest, u32), Option<Arc<callables::FnBound>>>,
    /// How many parameter bounds with associated-type bindings enclose the
    /// bound being read (`bounds.rs`).
    bound_depth: usize,
    /// The pattern being walked sits below a non-reference pattern that
    /// dereferenced a reference: rustc's default binding mode is `ref`.
    peeled: bool,
}
impl<'a> Component<'a> {
    fn new(key: &'a PcKey, plan: &'a SourceConstraintPlan, floating: bool) -> Self {
        Self {
            key,
            plan,
            floating,
            concrete: None,
            pending: None,
            unrecorded: false,
            via: Via::scalar(),
            expressions: BTreeSet::new(),
            pattern_members: BTreeSet::new(),
            bindings: BTreeMap::new(),
            work: Vec::new(),
            pattern_work: Vec::new(),
            patterns: Vec::new(),
            environment: None,
            literal_start: false,
            constructors: BTreeMap::new(),
            types: BTreeMap::new(),
            open: false,
            candidates: BTreeMap::new(),
            literal_family: None,
            dependents: BTreeMap::new(),
            deciding: Vec::new(),
            declared_depth: 0,
            goals: BTreeMap::new(),
            deferred: None,
            root_via: None,
            fn_bounds: BTreeMap::new(),
            bound_depth: 0,
            peeled: false,
        }
    }
    fn mark_unrecorded(&mut self) {
        self.unrecorded = true;
    }
    fn defer(&mut self, node: NodeId, demand: ConstraintDemand) {
        if let Some(log) = &mut self.deferred {
            log.push(demand);
        }
        if !self.via.strict {
            self.unrecorded = true;
        } else if self.pending.is_none() {
            self.pending = Some((node, demand));
        }
    }
    /// A finite Type never contains itself, so a projection (a path into
    /// the member's Type) is shorter than the Type is deep. Every level of
    /// that Type comes from a site of the body that nests (a constructor,
    /// call, borrow, loop head or pattern: `structural_depth`) and from the
    /// declared Types those sites relate, each at most `declared_depth` deep
    /// (the deepest declared Type this demand has read). A longer path could
    /// only come from a relation that is not the inverse of the one it was
    /// derived by, which in a cycle of the member graph would grow forever,
    /// so it states nothing.
    fn admit(&mut self, node: NodeId, via: &Via) -> bool {
        let bound = (self.plan.structural_depth() + 1).saturating_mul(self.declared_depth + 1);
        if via.steps.len() > bound {
            let current = std::mem::replace(&mut self.via, via.clone());
            self.defer(node, ConstraintDemand::ExpressionRelation);
            self.via = current;
            return false;
        }
        true
    }
    /// Note the nesting of a declared Type this demand reads.
    fn note_depth(&mut self, depth: usize) {
        self.declared_depth = self.declared_depth.max(depth);
    }
    fn expression_via(&mut self, node: Option<NodeId>, via: Via) {
        if let Some(node) = node
            && self.admit(node, &via)
            && self.expressions.insert((node, via.clone()))
        {
            self.work.push((node, via));
        }
    }
    fn expression(&mut self, node: Option<NodeId>) {
        let via = self.via.clone();
        self.expression_via(node, via);
    }
    fn pattern_via(&mut self, node: Option<NodeId>, via: Via) {
        if let Some(node) = node
            && self.admit(node, &via)
            && self.pattern_members.insert((node, via.clone()))
        {
            self.pattern_work.push((node, via));
        }
    }
    fn binding(&mut self, identity: Digest, pattern: NodeId) {
        let via = self.via.clone();
        if self
            .bindings
            .insert((identity, via.clone()), pattern)
            .is_none()
        {
            self.patterns.push((identity, pattern, via));
        }
    }
    fn constrain(&mut self, node: NodeId, primitive: PrimitiveType) -> Result<(), Diagnostic> {
        if self.open {
            self.candidate(node, TyTerm::Primitive(primitive));
            return Ok(());
        }
        if family(primitive) != Some(self.floating) {
            self.defer(node, ConstraintDemand::NumericFamily);
            return Ok(());
        }
        match self.concrete {
            Some((_, old)) if old != primitive => Err(fault("source numeric constraints disagree")),
            Some(_) => Ok(()),
            None => {
                self.concrete = Some((node, primitive));
                Ok(())
            }
        }
    }
    /// A closed Type an open variable's relation states at `node`.
    fn candidate(&mut self, node: NodeId, term: TyTerm) {
        self.candidates.entry(node).or_insert(term);
    }
    fn literal_reached(&mut self, floating: bool) {
        self.literal_family.get_or_insert(floating);
    }
    /// The Type a projection selects from a declared Type term; a nominal
    /// field step goes through the shared schema binder.
    fn project_term(
        &mut self,
        db: &mut Database,
        term: &TyTerm,
        steps: &[Step],
    ) -> QueryPoll<Option<TyTerm>> {
        self.note_depth(term_depth(term));
        let mut term = term.clone();
        for step in steps {
            // A written `_` is filled by the initializer: it states nothing.
            if term == hole() {
                return QueryPoll::Ready(Ok(Some(term)));
            }
            // Indexing and field access auto-deref their base, through
            // references and boxes alike.
            if matches!(step, Step::Element | Step::AutoDeref) {
                loop {
                    while let TyTerm::Ref(_, _, inner) = term {
                        term = *inner;
                    }
                    match ready!(self.box_contents(db, &term)) {
                        Some(contents) => term = contents,
                        None => break,
                    }
                }
            }
            // `*b` of a `Box<T>` is its builtin dereference: `T`.
            if *step == Step::Deref
                && let Some(contents) = ready!(self.box_contents(db, &term))
            {
                term = contents;
                continue;
            }
            // `*x` of another Type with an overloaded `Deref` (`Rc<T>`, a
            // user smart pointer): its `Target`.
            if *step == Step::Deref
                && matches!(term, TyTerm::Nominal(..))
                && let Some(target) = ready!(self.deref_target(db, &term))
            {
                term = target;
                continue;
            }
            // `v[i]` of a `Vec<T>` is its element place: `T`.
            if *step == Step::Element
                && let Some(element) = ready!(self.vec_elements(db, &term))
            {
                term = element;
                continue;
            }
            if *step == Step::Item {
                term = match ready!(self.iteration_item(db, &term)) {
                    iteration::ItemAnswer::Concrete(item) => item,
                    iteration::ItemAnswer::Part(path) => {
                        match ready!(self.project_term(db, &term, &path)) {
                            Some(item) => item,
                            None => return QueryPoll::Ready(Ok(None)),
                        }
                    }
                    // A Type built around the term's parts: the term's own
                    // parts fill its holes.
                    iteration::ItemAnswer::Structured { item, paths } => {
                        let mut parts = Vec::with_capacity(paths.len());
                        for path in &paths {
                            match ready!(self.project_term(db, &term, path)) {
                                Some(part) => parts.push(part),
                                None => return QueryPoll::Ready(Ok(None)),
                            }
                        }
                        iteration::fill_holes(&item, &parts)
                    }
                    iteration::ItemAnswer::Unknown => return QueryPoll::Ready(Ok(None)),
                };
                continue;
            }
            term = match (step, term) {
                (Step::AutoDeref, term) => term,
                (Step::TypeArgument(position), TyTerm::Nominal(_, args)) => {
                    match type_argument(&args, *position) {
                        Some(argument) => argument,
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                (Step::Deref, TyTerm::Ref(_, _, inner)) => *inner,
                // `*p` of a raw pointer is its pointee (E-R5-008).
                (Step::Deref, term)
                    if crate::traits::pointer_terms::as_raw_pointer(&term).is_some() =>
                {
                    match crate::traits::pointer_terms::as_raw_pointer(&term) {
                        Some((_, pointee)) => pointee.clone(),
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                (Step::Field(index), TyTerm::Tuple(mut elements)) => {
                    if (*index as usize) < elements.len() {
                        elements.swap_remove(*index as usize)
                    } else {
                        return QueryPoll::Ready(Ok(None));
                    }
                }
                (Step::Element, TyTerm::Array(inner, _) | TyTerm::Slice(inner)) => *inner,
                (
                    Step::Field(_) | Step::Named(_) | Step::Variant { .. },
                    TyTerm::Nominal(definition, args),
                ) => match ready!(self.nominal_field(db, definition, &args, step)) {
                    Some(field) => field,
                    None => return QueryPoll::Ready(Ok(None)),
                },
                _ => return QueryPoll::Ready(Ok(None)),
            };
        }
        QueryPoll::Ready(Ok(Some(term)))
    }
    /// `Some(T)` when `term` is `Box<T>`, the `owned_box` lang item (its
    /// declaration's identity in its own facade profile).
    fn box_contents(&mut self, db: &mut Database, term: &TyTerm) -> QueryPoll<Option<TyTerm>> {
        let TyTerm::Nominal(definition, args) = term else {
            return QueryPoll::Ready(Ok(None));
        };
        let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(*definition)) {
            QueryPoll::Ready(Ok(schema)) => schema,
            QueryPoll::Ready(Err(error)) if not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint box schema differs")));
        };
        if crate::facade::owned_box(&schema.header.profile) != *definition {
            return QueryPoll::Ready(Ok(None));
        }
        QueryPoll::Ready(Ok(type_argument(args, 0)))
    }
    /// `Some(T)` when `term` is `Vec<T>`, the growable-array lang item (its
    /// declaration's identity in its own facade profile).
    pub(super) fn vec_elements(
        &mut self,
        db: &mut Database,
        term: &TyTerm,
    ) -> QueryPoll<Option<TyTerm>> {
        let TyTerm::Nominal(definition, args) = term else {
            return QueryPoll::Ready(Ok(None));
        };
        let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(*definition)) {
            QueryPoll::Ready(Ok(schema)) => schema,
            QueryPoll::Ready(Err(error)) if not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint vec schema differs")));
        };
        if crate::facade::owned_vec(&schema.header.profile) != *definition {
            return QueryPoll::Ready(Ok(None));
        }
        QueryPoll::Ready(Ok(type_argument(args, 0)))
    }
    /// A declared Type term, projected by the current member's steps.
    fn constrain_term(
        &mut self,
        db: &mut Database,
        node: NodeId,
        term: &TyTerm,
        demand: ConstraintDemand,
    ) -> QueryPoll<()> {
        let steps = self.via.steps.clone();
        // A declared projection of concrete Types (`<S as Tr>::Out`) is the
        // Type its impl states.
        let term = &ready!(self.normalize_projections(db, term));
        match ready!(self.project_term(db, term, &steps)) {
            Some(TyTerm::Primitive(primitive)) => QueryPoll::Ready(self.constrain(node, primitive)),
            Some(term) if term == hole() => QueryPoll::Ready(Ok(())),
            // `!` coerces to every Type; it constrains nothing.
            Some(TyTerm::Never) => QueryPoll::Ready(Ok(())),
            // A numeric variable is never a reference itself: a reference to
            // a primitive at its position is an operand the built-in
            // operators auto-reference (`&u32 * 2`), whose other operand has
            // the primitive's Type.
            Some(TyTerm::Ref(_, _, inner))
                if !self.open && matches!(*inner, TyTerm::Primitive(_)) =>
            {
                match *inner {
                    TyTerm::Primitive(primitive) => {
                        QueryPoll::Ready(self.constrain(node, primitive))
                    }
                    _ => {
                        self.defer(node, demand);
                        QueryPoll::Ready(Ok(()))
                    }
                }
            }
            // A numeric variable unsized to a trait object must implement
            // its principal trait.
            Some(TyTerm::Dyn(principal)) if !self.open => {
                self.object_obligation(db, node, principal.0)
            }
            Some(term) if self.open && crate::frontend::call_contracts::closed_term(&term) => {
                self.candidate(node, term);
                QueryPoll::Ready(Ok(()))
            }
            _ => {
                self.defer(node, demand);
                QueryPoll::Ready(Ok(()))
            }
        }
    }
    fn fact(
        &self,
        node: NodeId,
    ) -> Result<&'a crate::syntax::source_constraints::SourceFact, Diagnostic> {
        let plan: &'a SourceConstraintPlan = self.plan;
        plan.fact(node)
            .ok_or_else(|| fault("numeric constraint source node absent"))
    }

    fn pattern_binding(&mut self, db: &mut Database, pattern: NodeId) -> QueryPoll<Option<Digest>> {
        pattern_binding(db, self.key, pattern)
    }

    /// A written Type at its canonical indexed occurrence: the written
    /// structure (`&T`, tuples, arrays, generic applications) around named
    /// leaves the ordinary entered Type goal selects, projected by the
    /// member's steps. Unresolved or nonprimitive stays pending.
    fn annotation(&mut self, db: &mut Database, owner: NodeId) -> QueryPoll<()> {
        let annotation = match self.fact(owner) {
            Ok(fact) => fact.annotation_node(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(annotation) = annotation else {
            self.defer(owner, ConstraintDemand::AnnotationType);
            return QueryPoll::Ready(Ok(()));
        };
        let term = ready!(self.written_term(db, annotation));
        self.constrain_term(db, owner, &term, ConstraintDemand::AnnotationType)
    }
    /// The Type term of a written Type node. A leaf the entered Type
    /// service cannot select is an open part (never a guess).
    fn written_term(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<TyTerm> {
        let plan: &'a SourceConstraintPlan = self.plan;
        let open = || {
            TyTerm::Infer(crate::traits::term::InferVarKey {
                transaction: Digest::default(),
                ordinal: 0,
            })
        };
        QueryPoll::Ready(Ok(match plan.type_shape(node) {
            Some(TypeShape::Parenthesis(inner)) => ready!(self.written_term(db, *inner)),
            Some(TypeShape::Reference(inner)) => TyTerm::Ref(
                crate::traits::term::RegionTerm::Static,
                false,
                Box::new(ready!(self.written_term(db, *inner))),
            ),
            Some(TypeShape::RawPointer(mutable, inner)) => {
                crate::traits::pointer_terms::raw_pointer(
                    *mutable,
                    ready!(self.written_term(db, *inner)),
                )
            }
            Some(TypeShape::Element(inner)) => {
                TyTerm::Slice(Box::new(ready!(self.written_term(db, *inner))))
            }
            Some(TypeShape::Tuple(elements)) => {
                let mut members = Vec::with_capacity(elements.len());
                for element in elements {
                    members.push(ready!(self.written_term(db, *element)));
                }
                TyTerm::Tuple(members)
            }
            Some(TypeShape::Infer) => hole(),
            // A trait object is its principal trait's schema, as the
            // local annotation service reads it.
            Some(TypeShape::Dyn(principal)) => {
                match crate::frontend::nominal_constructors::dyn_annotation_principal(
                    db,
                    &self.key.body,
                    *principal,
                ) {
                    QueryPoll::Ready(Ok(principal)) => {
                        TyTerm::Dyn(crate::traits::term::DynSchemaKey(principal.identity))
                    }
                    QueryPoll::Ready(Err(error)) if entered_type_refusal(&error) => open(),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            Some(TypeShape::Generic(base, arguments)) => {
                // The local annotation service's own generic base selection.
                match crate::frontend::nominal_constructors::generic_annotation_base(
                    db,
                    &self.key.body,
                    *base,
                ) {
                    QueryPoll::Ready(Ok(selected))
                        if selected.type_parameters == arguments.len() =>
                    {
                        let mut args = Vec::with_capacity(arguments.len());
                        for argument in arguments {
                            args.push(crate::traits::term::GenericArg::Type(ready!(
                                self.written_term(db, *argument)
                            )));
                        }
                        TyTerm::Nominal(selected.identity, args)
                    }
                    QueryPoll::Ready(Ok(_)) => open(),
                    QueryPoll::Ready(Err(error)) if entered_type_refusal(&error) => open(),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            None => match db.fetch(
                &SyntaxProvider,
                QueryKey::EnteredType(entry(self.key, node)),
            ) {
                QueryPoll::Ready(Ok(goal)) => match goal.value.as_ref() {
                    QueryValue::EnteredType(goal) => goal.source_type().source().clone(),
                    _ => {
                        return QueryPoll::Ready(Err(fault(
                            "constraint annotation query variant differs",
                        )));
                    }
                },
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                // Unresolved names, aliases and generic parameters are open.
                QueryPoll::Ready(Err(error)) if entered_type_refusal(&error) => open(),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            },
        }))
    }

    /// The caller body's completed source environment: the same name graph
    /// and sources the run machine uses for call contracts. Retained through
    /// the ordinary SourceLocalPlan (and its BodyEnvironment input) query.
    fn environment(
        &mut self,
        db: &mut Database,
    ) -> QueryPoll<Option<Arc<crate::expand::source_discovery::CompletedSourceEnvironment>>> {
        if let Some(environment) = &self.environment {
            return QueryPoll::Ready(Ok(Some(Arc::clone(environment))));
        }
        let plan = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceLocalPlan(self.key.body.clone())
        ));
        let QueryValue::SourceLocalPlan(plan) = plan.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint source plan variant differs")));
        };
        let Some(environment) = plan.environment() else {
            return QueryPoll::Ready(Ok(None));
        };
        let environment = Arc::clone(environment.macros().source_environment());
        self.environment = Some(Arc::clone(&environment));
        QueryPoll::Ready(Ok(Some(environment)))
    }

    /// rustc's `ExpectCastableToType` for an unsuffixed literal operand:
    /// an integer literal takes an integer target (`u8` for `char`), a float
    /// literal a float target; any other target leaves it a fresh variable.
    fn cast_expectation(&mut self, db: &mut Database, cast: NodeId) -> QueryPoll<()> {
        let annotation = match self.fact(cast) {
            Ok(fact) => fact.annotation_node(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(annotation) = annotation else {
            self.defer(cast, ConstraintDemand::AnnotationType);
            return QueryPoll::Ready(Ok(()));
        };
        let goal = match db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredType(entry(self.key, annotation)),
        ) {
            QueryPoll::Ready(Ok(goal)) => goal,
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            QueryPoll::Ready(Err(error)) if entered_type_refusal(&error) => {
                self.defer(cast, ConstraintDemand::AnnotationType);
                return QueryPoll::Ready(Ok(()));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        };
        let QueryValue::EnteredType(goal) = goal.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint annotation query variant differs")));
        };
        let expected = match (goal.source_type().primitive(), self.floating) {
            (Some(PrimitiveType::Char), false) => Some(PrimitiveType::U8),
            (Some(primitive), floating) if family(primitive) == Some(floating) => Some(primitive),
            _ => None,
        };
        if let Some(expected) = expected
            && let Err(error) = self.constrain(cast, expected)
        {
            return QueryPoll::Ready(Err(error));
        }
        QueryPoll::Ready(Ok(()))
    }

    /// The declared result Type of an actual selected function definition,
    /// local or module/impl-owned, through the common declared-result goal.
    /// `None` is a missing goal (an unreadable header, a non-function
    /// selection or no environment for a module declaration).
    fn result(&mut self, db: &mut Database, node: NodeId, definition: Digest) -> QueryPoll<()> {
        match ready!(self.result_term(db, node, definition)) {
            Some(term) => {
                self.constrain_declared(db, node, definition, &term, ConstraintDemand::ResultType)
            }
            None => QueryPoll::Ready(Ok(())),
        }
    }
    /// A selected function's declared parameter or result Type projected by
    /// the current member's steps: a concrete leaf constrains; the callee's
    /// own type parameter relates its other positions (and its bounds).
    fn constrain_declared(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
        term: &TyTerm,
        demand: ConstraintDemand,
    ) -> QueryPoll<()> {
        let steps = self.via.steps.clone();
        let is_call = matches!(
            self.plan.fact(node).map(|fact| fact.kind()),
            Some(FactKind::Call | FactKind::MethodCall | FactKind::LangCall)
        );
        // The callee's own parameters are bound by the call's operands, so an
        // associated-type projection of them (`<T as Get>::Value`) is the
        // Type the selected impl states.
        let instantiated;
        let term = if super::associated_types::contains_projection(term) {
            instantiated = ready!(self.instantiated_declared(db, node, definition, term));
            &instantiated
        } else {
            term
        };
        match ready!(self.project_parametric(db, term, &steps)) {
            generics::TermProjection::Parameter(key, remaining) if is_call => {
                self.relate_parameter(db, node, definition, key, remaining)
            }
            generics::TermProjection::Leaf(_) => self.constrain_term(db, node, term, demand),
            _ => {
                self.defer(node, demand);
                QueryPoll::Ready(Ok(()))
            }
        }
    }
    /// The declared result Type term of a selected function, or `None`
    /// after deferring `node` on the missing goal.
    fn result_term(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
    ) -> QueryPoll<Option<TyTerm>> {
        self.goal(db, node, definition, None)
    }
    /// A selected function's declared parameter (`Some(ordinal)`) or result
    /// goal, read once per demand: later relations reuse the answer and
    /// replay the demand that answered it.
    fn goal(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
        ordinal: Option<u32>,
    ) -> QueryPoll<Option<TyTerm>> {
        if let Some((term, demands)) = self.goals.get(&(definition, ordinal)).cloned() {
            for demand in demands {
                self.defer(node, demand);
            }
            return QueryPoll::Ready(Ok(term));
        }
        let outer = self.deferred.replace(Vec::new());
        let term = match ordinal {
            None => self.result_term_uncached(db, node, definition),
            Some(ordinal) => self.parameter_term_uncached(db, node, definition, ordinal),
        };
        let demands = std::mem::replace(&mut self.deferred, outer).unwrap_or_default();
        if let Some(outer) = &mut self.deferred {
            outer.extend(demands.iter().copied());
        }
        let term = ready!(term);
        if let Some(term) = &term {
            self.note_depth(term_depth(term));
        }
        self.goals
            .insert((definition, ordinal), (term.clone(), demands));
        QueryPoll::Ready(Ok(term))
    }
    fn result_term_uncached(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
    ) -> QueryPoll<Option<TyTerm>> {
        let goal = if let Some(environment) = ready!(self.environment(db)) {
            let sources: Vec<_> = environment.sources().iter().collect();
            super::source_return_goals::prepare(db, environment.names(), &sources, definition)
        } else if is_local(db, definition) {
            // Without a completed environment only a local declaration's
            // retained memo can answer.
            match db.fetch(&SyntaxProvider, QueryKey::DeclaredReturnGoal(definition)) {
                QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                    QueryValue::DeclaredReturnGoal(goal) => {
                        QueryPoll::Ready(Ok(goal.goal().clone()))
                    }
                    _ => {
                        return QueryPoll::Ready(Err(fault(
                            "constraint result query variant differs",
                        )));
                    }
                },
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            }
        } else {
            self.defer(node, ConstraintDemand::ModuleEnvironment);
            return QueryPoll::Ready(Ok(None));
        };
        let goal = match goal {
            QueryPoll::Ready(Ok(goal)) => goal,
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            QueryPoll::Ready(Err(error)) if not_closed(&error) => {
                self.defer(node, ConstraintDemand::ResultType);
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        };
        let ReturnTypeGoal::Ready(declared) = goal else {
            self.defer(node, ConstraintDemand::ResultType);
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(Some(declared.source().clone())))
    }

    /// The actual selected callee of a call through the ordinary hygienic
    /// Value goal (module and generated `$crate` paths included). A
    /// type-relative path (`P::f`) awaits the common member goal and stays
    /// pending.
    /// Relate a closure's body value to the calls that return it: a closure
    /// called where it is written, or every call of the local it initializes
    /// (a closure has a unique type, so that local only ever calls it).
    /// Calls the source does not relate here (a closure passed to a generic
    /// callee or stored) add no constraint: the closed component falls back
    /// as rustc's would without that callee's `Fn*` bound.
    fn closure_calls(&mut self, db: &mut Database, closure: NodeId) -> QueryPoll<()> {
        let plan: &'a SourceConstraintPlan = self.plan;
        // A closure written for a parameter bounded by `Fn*(..) -> R` returns
        // that `R`.
        ready!(self.closure_result_bound(db, closure));
        for edge in plan.uses(closure) {
            let Some(parent) = plan.fact(edge.parent) else {
                continue;
            };
            match parent.kind() {
                FactKind::Call if edge.ordinal == 0 => self.expression(Some(edge.parent)),
                FactKind::Parenthesis | FactKind::Block => {
                    ready!(self.closure_calls(db, edge.parent))
                }
                FactKind::Let if edge.ordinal == 1 => {
                    let Some(pattern) = parent.operand(0) else {
                        continue;
                    };
                    let Some(identity) = ready!(self.pattern_binding(db, pattern)) else {
                        continue;
                    };
                    let Some((_, row)) = plan.name_of(pattern) else {
                        continue;
                    };
                    for use_node in row.paths().iter().copied() {
                        let observed = ready!(db.fetch(
                            &SyntaxProvider,
                            QueryKey::EnteredValue(entry(self.key, use_node))
                        ));
                        let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
                            return QueryPoll::Ready(Err(fault(
                                "closure use query variant differs",
                            )));
                        };
                        if selected_local(observed) != Some(identity) {
                            continue;
                        }
                        for call in plan.uses(use_node) {
                            if call.ordinal == 0
                                && plan
                                    .fact(call.parent)
                                    .is_some_and(|fact| fact.kind() == FactKind::Call)
                            {
                                self.expression(Some(call.parent));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        QueryPoll::Ready(Ok(()))
    }
    /// The closure expression a callee expression evaluates to, when the
    /// source states it: a closure written there (through parentheses), a
    /// local declared with one, or a call of a closure whose body is one.
    fn closure_of(
        &mut self,
        db: &mut Database,
        node: NodeId,
        depth: usize,
    ) -> QueryPoll<Option<NodeId>> {
        let plan: &'a SourceConstraintPlan = self.plan;
        let Some(fact) = plan.fact(node) else {
            return QueryPoll::Ready(Ok(None));
        };
        // Every step descends into strictly smaller syntax.
        if depth > plan.facts_len() {
            return QueryPoll::Ready(Ok(None));
        }
        match fact.kind() {
            FactKind::Closure => QueryPoll::Ready(Ok(Some(node))),
            // A block's value is its tail expression (`let f = {|i| i};`).
            FactKind::Parenthesis | FactKind::Block => match fact.operand(0) {
                Some(inner) => self.closure_of(db, inner, depth + 1),
                None => QueryPoll::Ready(Ok(None)),
            },
            FactKind::Call => {
                let Some(callee) = fact.operand(0) else {
                    return QueryPoll::Ready(Ok(None));
                };
                let Some(closure) = ready!(self.closure_of(db, callee, depth + 1)) else {
                    return QueryPoll::Ready(Ok(None));
                };
                match plan.fact(closure).and_then(|fact| fact.operand(0)) {
                    Some(body) => self.closure_of(db, body, depth + 1),
                    None => QueryPoll::Ready(Ok(None)),
                }
            }
            FactKind::Path => {
                let selected = ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::EnteredValue(entry(self.key, node))
                ));
                let QueryValue::EnteredValue(selected) = selected.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault("closure callee query variant differs")));
                };
                match selected_local(selected) {
                    Some(identity) => self.bound_closure(db, node, identity),
                    None => QueryPoll::Ready(Ok(None)),
                }
            }
            _ => QueryPoll::Ready(Ok(None)),
        }
    }
    /// The closure expression a local is declared with (`let f = |..| ..;`).
    fn bound_closure(
        &mut self,
        db: &mut Database,
        use_node: NodeId,
        identity: Digest,
    ) -> QueryPoll<Option<NodeId>> {
        let plan: &'a SourceConstraintPlan = self.plan;
        let Some(pattern) = ready!(binding_pattern(db, self.key, plan, use_node, identity)) else {
            return QueryPoll::Ready(Ok(None));
        };
        for edge in plan.uses(pattern) {
            let Some(statement) = plan.fact(edge.parent) else {
                continue;
            };
            if statement.kind() != FactKind::Let || edge.ordinal != 0 {
                continue;
            }
            // The closure may sit in parentheses or as a block's tail value.
            let mut initializer = statement.operand(1);
            while let Some(node) = initializer
                && plan.fact(node).is_some_and(|fact| {
                    matches!(fact.kind(), FactKind::Parenthesis | FactKind::Block)
                })
            {
                initializer = plan.fact(node).and_then(|fact| fact.operand(0));
            }
            if let Some(initializer) = initializer
                && plan
                    .fact(initializer)
                    .is_some_and(|fact| fact.kind() == FactKind::Closure)
            {
                return QueryPoll::Ready(Ok(Some(initializer)));
            }
        }
        QueryPoll::Ready(Ok(None))
    }
    /// The function item a local is declared with (`let f = path;`, no
    /// annotation): the declaration the path's ordinary Value goal selects.
    fn bound_function_item(
        &mut self,
        db: &mut Database,
        use_node: NodeId,
        identity: Digest,
    ) -> QueryPoll<Option<Digest>> {
        let plan: &'a SourceConstraintPlan = self.plan;
        let Some(pattern) = ready!(binding_pattern(db, self.key, plan, use_node, identity)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut initializer = plan.uses(pattern).iter().find_map(|edge| {
            let statement = plan.fact(edge.parent)?;
            (edge.ordinal == 0
                && statement.kind() == FactKind::Let
                && statement.annotation().is_none())
            .then(|| statement.operand(1))
            .flatten()
        });
        // Parentheses keep their operand's value; each step descends.
        while let Some(node) = initializer
            && plan
                .fact(node)
                .is_some_and(|fact| fact.kind() == FactKind::Parenthesis)
        {
            initializer = plan.fact(node).and_then(|fact| fact.operand(0));
        }
        let Some(initializer) = initializer else {
            return QueryPoll::Ready(Ok(None));
        };
        if plan.fact(initializer).map(|fact| fact.kind()) != Some(FactKind::Path) {
            return QueryPoll::Ready(Ok(None));
        }
        let selected = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredValue(entry(self.key, initializer))
        ));
        let QueryValue::EnteredValue(selected) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("function item query variant differs")));
        };
        if selection_pending(selected) || selected.local_binding().is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        match selected.declaration() {
            Some(declaration) => Self::selected_function(db, declaration),
            None => QueryPoll::Ready(Ok(None)),
        }
    }
    /// Source function identity from an actual retained declaration, shared
    /// by direct path calls and locals initialized with function items.
    fn selected_function(
        db: &mut Database,
        declaration: &crate::frontend::source_value_goals::SelectedValueDeclaration,
    ) -> QueryPoll<Option<Digest>> {
        let parsed = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::ParseFile(declaration.source().clone())
        ));
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint callee source outcome differs")));
        };
        let item = match parsed.syntax_at(declaration.context().range, |node| {
            ra_ap_syntax::ast::Item::can_cast(node.kind())
        }) {
            Ok(Some(item)) => item,
            Ok(None) => {
                return QueryPoll::Ready(Err(fault("constraint callee actual declaration absent")));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        QueryPoll::Ready(Ok(
            ra_ap_syntax::ast::Fn::can_cast(item.kind()).then(|| declaration.identity())
        ))
    }
    /// A call through a callee value: a literal's demand keeps its reached
    /// (run-time) context; a binding's component has no static Type from it.
    fn runtime_callee(&mut self, callee: NodeId) {
        if self.literal_start {
            self.defer(callee, ConstraintDemand::SelectedDeclaration);
        } else {
            self.mark_unrecorded();
        }
    }
    /// `module::member` where the root the Value goal selected is a module
    /// (`use std::mem; mem::swap`): the member the module route selects from
    /// the module's path spelled at its defining crate, as the dispatcher does.
    fn module_rooted_callee(
        &mut self,
        db: &mut Database,
        module: Digest,
        member: &str,
    ) -> QueryPoll<Option<Callee>> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let Some(item) = sources
            .iter()
            .find_map(|source| source.outline_item(module))
            .filter(|item| item.def_key.item_kind == crate::source::manifest::ItemKind::Module)
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut segments: Vec<&str> = item
            .def_key
            .module_path
            .iter()
            .map(String::as_str)
            .collect();
        segments.push(&item.def_key.declared_name);
        let spelled = format!(
            "{}::{}::{member}",
            super::callables::dollar_crate_segment(item.def_key.crate_key),
            segments.join("::")
        );
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let resolver = match super::callables::CallResolver::new(environment.names(), &sources) {
            Ok(resolver) => resolver.with_members(Some(&members)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match resolver.source_function(db, self.key.body.definition, &spelled) {
            QueryPoll::Ready(Ok(super::callables::CallSelection::Source(callable))) => {
                QueryPoll::Ready(Ok(Some(Callee::Source(callable.identity()))))
            }
            QueryPoll::Ready(Ok(super::callables::CallSelection::Facade(function, _))) => {
                QueryPoll::Ready(Ok(Some(Callee::Facade(function))))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) if not_closed(&error) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn callee(&mut self, db: &mut Database, call: NodeId) -> QueryPoll<Option<Callee>> {
        let callee = match self.fact(call) {
            Ok(fact) => fact.operand(0),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(callee) = callee else {
            self.defer(call, ConstraintDemand::SelectedDeclaration);
            return QueryPoll::Ready(Ok(None));
        };
        // A callee value (a closure or function item in a local, parameter
        // or any non-path expression) selects its callee only at run time:
        // its parameter and result Types are the reached values' own, so the
        // component stays unconstrained (no fallback) rather than pending.
        if !matches!(
            self.fact(callee).map(|fact| fact.kind()),
            Ok(FactKind::Path)
        ) {
            if let Some(closure) = ready!(self.closure_of(db, callee, 0)) {
                return QueryPoll::Ready(Ok(Some(Callee::Closure(closure))));
            }
            self.runtime_callee(callee);
            return QueryPoll::Ready(Ok(None));
        }
        let selected = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredValue(entry(self.key, callee))
        ));
        let QueryValue::EnteredValue(selected) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint callee query variant differs")));
        };
        let local = selected_local(selected);
        if selection_pending(selected) && local.is_none() {
            self.defer(callee, ConstraintDemand::SelectedDeclaration);
            return QueryPoll::Ready(Ok(None));
        }
        if let Some(function) = selected.facade() {
            return QueryPoll::Ready(Ok(Some(Callee::Facade(function))));
        }
        if let Some(identity) = local {
            if let Some(closure) = ready!(self.bound_closure(db, callee, identity)) {
                return QueryPoll::Ready(Ok(Some(Callee::Closure(closure))));
            }
            // A local declared with a function item (`let rs = ten;`) has
            // that function's signature.
            if let Some(function) = ready!(self.bound_function_item(db, callee, identity)) {
                return QueryPoll::Ready(Ok(Some(Callee::Source(function))));
            }
            // A parameter of the executing function whose Type states the
            // signature it is called with.
            if ready!(self.binding_pattern(db, callee, identity)).is_none()
                && let Some(bound) = ready!(self.parameter_fn_bound(db, callee))
            {
                return QueryPoll::Ready(Ok(Some(Callee::Bound(bound))));
            }
            self.runtime_callee(callee);
            return QueryPoll::Ready(Ok(None));
        }
        // `Root::member` through a block item or import: the dispatcher selects
        // the member against its root at run time (the declaration here is the
        // root itself, an enum or module, not a callable).
        if let Some(member) = selected.rooted() {
            if let Some(declaration) = selected.declaration() {
                let identity = declaration.identity();
                if let Some(found) = ready!(self.module_rooted_callee(db, identity, member)) {
                    return QueryPoll::Ready(Ok(Some(found)));
                }
            }
            self.runtime_callee(callee);
            return QueryPoll::Ready(Ok(None));
        }
        let Some(declaration) = selected.declaration() else {
            self.defer(callee, ConstraintDemand::SelectedDeclaration);
            return QueryPoll::Ready(Ok(None));
        };
        // A selected Value declaration can be a constructor whose field
        // Types are not yet closed. It is not then a function: only the
        // declaration's actual retained Fn occurrence has a signature.
        let Some(function) = ready!(Self::selected_function(db, declaration)) else {
            self.defer(call, ConstraintDemand::SelectedDeclaration);
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(Some(Callee::Source(function))))
    }

    fn argument(&mut self, db: &mut Database, call: NodeId, ordinal: u32) -> QueryPoll<()> {
        let definition = match ready!(self.callee(db, call)) {
            Some(Callee::Source(definition)) => definition,
            // The argument binds the closure's parameter of that position.
            Some(Callee::Closure(closure)) => {
                let parameter = match self.fact(closure) {
                    Ok(fact) => fact.operand(ordinal as usize),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                if let Some(parameter) = parameter
                    && let Some(identity) = ready!(self.pattern_binding(db, parameter))
                {
                    self.binding(identity, parameter);
                }
                return QueryPoll::Ready(Ok(()));
            }
            // `must_use<T>(T) -> T` gives its result the argument's Type.
            Some(Callee::Facade(LibraryFunction::MustUse)) => {
                self.expression(Some(call));
                return QueryPoll::Ready(Ok(()));
            }
            // The written parameter Type of the callable's signature.
            Some(Callee::Bound(bound)) => {
                return self.bound_argument(db, call, &bound, ordinal - 1);
            }
            // No other facade parameter is numeric; `assert_failed`'s `&T`
            // and `&U` are generic formatting operands.
            Some(Callee::Facade(_)) | None => return QueryPoll::Ready(Ok(())),
        };
        self.declared_parameter(db, call, definition, ordinal - 1)
    }
    /// Relate the current member to parameter `ordinal` of a selected
    /// function's declared signature.
    fn declared_parameter(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
        ordinal: u32,
    ) -> QueryPoll<()> {
        let Some(term) = ready!(self.parameter_term(db, call, definition, ordinal)) else {
            return QueryPoll::Ready(Ok(()));
        };
        // A reference argument may deref-coerce to the declared reference
        // Type (`&Vec<T>` to `&[T]`): the variable is then at the coerced
        // Type's position.
        match ready!(self.coerce_argument(db, call, ordinal, &term)) {
            Some(coerced) => {
                let current = self.via.clone();
                for via in coerced {
                    self.via = via;
                    ready!(self.constrain_declared(
                        db,
                        call,
                        definition,
                        &term,
                        ConstraintDemand::ParameterType,
                    ));
                }
                self.via = current;
                QueryPoll::Ready(Ok(()))
            }
            None => self.constrain_declared(
                db,
                call,
                definition,
                &term,
                ConstraintDemand::ParameterType,
            ),
        }
    }
    /// Parameter `ordinal`'s declared Type term, or `None` after deferring
    /// `node` on the missing goal.
    fn parameter_term(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
        ordinal: u32,
    ) -> QueryPoll<Option<TyTerm>> {
        self.goal(db, node, definition, Some(ordinal))
    }
    fn parameter_term_uncached(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
        ordinal: u32,
    ) -> QueryPoll<Option<TyTerm>> {
        let call = node;
        let goal = if let Some(environment) = ready!(self.environment(db)) {
            let sources: Vec<_> = environment.sources().iter().collect();
            super::source_parameter_goals::prepare(
                db,
                environment.names(),
                &sources,
                definition,
                ordinal as usize,
            )
        } else if is_local(db, definition) {
            match db.fetch(
                &SyntaxProvider,
                QueryKey::DeclaredParameterGoal {
                    definition,
                    ordinal,
                },
            ) {
                QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                    QueryValue::DeclaredParameterGoal(goal) => {
                        QueryPoll::Ready(Ok(goal.goal().clone()))
                    }
                    _ => {
                        return QueryPoll::Ready(Err(fault(
                            "constraint parameter query variant differs",
                        )));
                    }
                },
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            }
        } else {
            self.defer(call, ConstraintDemand::ModuleEnvironment);
            return QueryPoll::Ready(Ok(None));
        };
        let goal = match goal {
            QueryPoll::Ready(Ok(goal)) => goal,
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            // An excess argument, a constructor or an unreadable header.
            QueryPoll::Ready(Err(error)) if not_closed(&error) => {
                self.defer(call, ConstraintDemand::ParameterType);
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        };
        let ParameterTypeGoal::Ready(goal) = goal else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(Some(goal.source().clone())))
    }

    /// A read of the owning function's own parameter: its declared Type
    /// through the common parameter goal. The parameter is selected by its
    /// written identifier pattern (`x` / `mut x`); any other parameter
    /// pattern stays a missing goal.
    fn parameter(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        match ready!(self.own_parameter_term(db, node)) {
            Some(term) => self.constrain_term(db, node, &term, ConstraintDemand::ParameterType),
            None => QueryPoll::Ready(Ok(())),
        }
    }
    /// The declared Type of the owning function's parameter a path reads:
    /// its written identifier pattern (`x` / `mut x`) or the receiver for
    /// `self`. Any other parameter pattern stays a missing goal.
    fn own_parameter_term(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<Option<TyTerm>> {
        let plan: &'a SourceConstraintPlan = self.plan;
        let Some((name, _)) = plan.name_of(node) else {
            self.defer(node, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(None));
        };
        let definition = self.key.body.definition;
        let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            QueryPoll::Ready(Err(error)) if not_closed(&error) => {
                self.defer(node, ConstraintDemand::Binding);
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        };
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint parameter signature differs")));
        };
        let written = |parameter: &crate::syntax::signatures::ParameterRecipe| {
            if parameter.receiver {
                return name == "self";
            }
            let tokens: Vec<&str> = parameter
                .pattern
                .iter()
                .map(|token| token.text.as_str())
                .collect();
            let spelled = match tokens.as_slice() {
                [only] | ["mut", only] => only.strip_prefix("r#").unwrap_or(only),
                _ => return false,
            };
            spelled == name
        };
        let mut ordinals = signature
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| written(parameter))
            .map(|(ordinal, _)| ordinal);
        let (Some(ordinal), None) = (ordinals.next(), ordinals.next()) else {
            self.defer(node, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(None));
        };
        let Ok(ordinal) = u32::try_from(ordinal) else {
            self.defer(node, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(None));
        };
        self.parameter_term(db, node, definition, ordinal)
    }

    /// What the expression itself says about its own Type.
    fn inward(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // A `for` head's Item: selected from the expression's static Type;
        // value-carrying syntax (paths, blocks, calls, ..) also passes it on.
        if self.via.steps.first() == Some(&Step::Item) {
            let selected = ready!(self.item_inward(db, node));
            let passes = matches!(
                fact.kind(),
                FactKind::Path
                    | FactKind::Parenthesis
                    | FactKind::Block
                    | FactKind::If
                    | FactKind::Match
                    | FactKind::Call
                    | FactKind::MethodCall
                    | FactKind::LangCall
                    | FactKind::Field
                    | FactKind::Index
                    | FactKind::IndexRange
            );
            if !passes {
                if !selected {
                    self.defer(node, ConstraintDemand::ExpressionRelation);
                }
                return QueryPoll::Ready(Ok(()));
            }
        }
        let scalar = self.via.is_scalar();
        // An open variable also takes the closed Type the expression
        // itself states (a typed literal, a tuple of them, a local whose
        // initializer states one).
        if self.open {
            ready!(self.static_candidate(db, node));
        }
        match fact.kind() {
            // A lexically typed literal states its Type (above) and nothing else.
            FactKind::Literal if self.open && (fact.suffix().is_some() || fact.flag()) => {}
            FactKind::Integer | FactKind::Float if scalar && self.open => {
                self.literal_reached(fact.kind() == FactKind::Float)
            }
            FactKind::Integer if scalar && !self.floating => {}
            FactKind::Float if scalar && self.floating => {}
            FactKind::SuffixedInteger | FactKind::SuffixedFloat if scalar => match fact.suffix() {
                Some(primitive) => {
                    if let Err(error) = self.constrain(node, primitive) {
                        return QueryPoll::Ready(Err(error));
                    }
                }
                None => self.defer(node, ConstraintDemand::NumericFamily),
            },
            FactKind::Path => {
                let observed = ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::EnteredValue(entry(self.key, node))
                ));
                let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault("binding use query variant differs")));
                };
                let local = selected_local(observed);
                if selection_pending(observed) && local.is_none() {
                    // A fieldless constructor named as a value (`None`) is
                    // a constructed value of its declaration's Type.
                    match ready!(self.constructor(db, ConstructorSite::Expression(node))) {
                        Some(constructed) => {
                            return self.constructed_inward(db, node, &constructed, &[]);
                        }
                        None => self.defer(node, ConstraintDemand::ValueUse),
                    }
                } else if let Some(identity) = local {
                    if !self.bindings.contains_key(&(identity, self.via.clone())) {
                        let pattern = ready!(self.binding_pattern(db, node, identity));
                        match pattern {
                            Some(pattern) => self.binding(identity, pattern),
                            // A hygienic local with no indexed pattern in
                            // this body is one of its function's parameters.
                            None => ready!(self.parameter(db, node)),
                        }
                    }
                } else if let Some(declaration) = observed.declaration() {
                    // A constant's or static's declared Type.
                    let identity = declaration.identity();
                    match ready!(self.declared_item_type(db, observed, identity)) {
                        Some(term) => ready!(self.constrain_term(
                            db,
                            node,
                            &term,
                            ConstraintDemand::SelectedDeclaration
                        )),
                        None => self.defer(node, ConstraintDemand::SelectedDeclaration),
                    }
                } else {
                    // A function item or library value: no numeric Type here.
                    self.defer(node, ConstraintDemand::SelectedDeclaration);
                }
            }
            FactKind::Parenthesis | FactKind::Block => match fact.operand(0) {
                Some(child) => self.expression(Some(child)),
                None => self.defer(node, ConstraintDemand::ExpressionRelation),
            },
            FactKind::Prefix => match fact.operation() {
                Some(SourceOperator::Negate | SourceOperator::Not) if scalar => {
                    self.expression(fact.operand(0))
                }
                Some(SourceOperator::Deref) => {
                    let via = self.via.wrap(Step::Deref);
                    self.expression_via(fact.operand(0), via);
                }
                _ => self.defer(node, ConstraintDemand::ExpressionRelation),
            },
            FactKind::Binary if scalar => match fact.operation() {
                Some(SourceOperator::Arithmetic) => {
                    self.expression(fact.operand(0));
                    self.expression(fact.operand(1));
                }
                Some(SourceOperator::Shift) => self.expression(fact.operand(0)),
                // bool and unit results cannot hold a numeric variable.
                _ => self.defer(node, ConstraintDemand::ExpressionRelation),
            },
            FactKind::If => match (fact.operand(1), fact.operand(2)) {
                (Some(then), Some(otherwise)) => {
                    self.expression(Some(then));
                    self.expression(Some(otherwise));
                }
                _ => self.defer(node, ConstraintDemand::ExpressionRelation),
            },
            // The arms' values are the match's value.
            FactKind::Match => {
                let via = self.via.loose();
                for body in fact.operands().iter().skip(2).step_by(2) {
                    self.expression_via(*body, via.clone());
                }
            }
            FactKind::Borrow => match self.via.steps.first() {
                Some(Step::Deref) => {
                    let via = self.via.unwrap(Step::Deref).expect("checked step");
                    self.expression_via(fact.operand(0), via)
                }
                // Autoderef of `&e` continues into `e`.
                Some(Step::AutoDeref) => {
                    let via = self.via.loose();
                    self.expression_via(fact.operand(0), via)
                }
                _ => self.defer(node, ConstraintDemand::ExpressionRelation),
            },
            // `base.i` / `base.name` is that field of the auto-dereferenced
            // base (a tuple position, or a struct field by written name).
            FactKind::Field => {
                let step = match (fact.field(), fact.member()) {
                    (Some(position), _) => Some(Step::Field(position)),
                    (None, Some(row)) => Some(Step::Named(row)),
                    (None, None) => None,
                };
                match step {
                    Some(step) => {
                        let via = self.via.wrap(step).wrap(Step::AutoDeref);
                        self.expression_via(fact.operand(0), via);
                    }
                    None => self.defer(node, ConstraintDemand::ExpressionRelation),
                }
            }
            // `S { f: e, ..base }`: the selected field's initializer and
            // declared Type; `..base` has the record's own Type.
            FactKind::Record => {
                if fact.flag() {
                    let rows = self.plan.record_fields(node).map_or(0, <[u32]>::len);
                    let via = self.via.clone();
                    self.expression_via(fact.operand(rows), via);
                }
                ready!(self.record_inward(db, node));
            }
            // Every element has the array's element Type (an array value
            // is not a reference; autoderef selects it).
            FactKind::Array => match self
                .via
                .unwrap(Step::AutoDeref)
                .unwrap_or_else(|| self.via.clone())
                .unwrap(Step::Element)
            {
                Some(via) => {
                    for element in fact.operands() {
                        self.expression_via(*element, via.clone());
                    }
                }
                None => self.defer(node, ConstraintDemand::ExpressionRelation),
            },
            // `base[index]` is one element of the auto-dereferenced base
            // (a range index is `IndexRange`).
            FactKind::Index => {
                if ready!(self.user_index_output(db, node)).is_some() {
                    // The base's own `Index` impl states the result Type
                    // (`static_candidate` above); no element step projects
                    // the base, so a variable inside the result stays open.
                    if !self.via.steps.is_empty() {
                        self.mark_unrecorded();
                    }
                } else if ready!(self.index_is_element(db, node)) {
                    let via = self.via.wrap(Step::Element).wrap(Step::AutoDeref);
                    self.expression_via(fact.operand(0), via);
                } else {
                    self.defer(node, ConstraintDemand::ExpressionRelation);
                }
            }
            // `base[range]` is a slice of the auto-dereferenced base: both
            // hold the same elements.
            FactKind::IndexRange => {
                // Autoderef repeats as needed: a leading one is already it.
                let via = match self.via.steps.first() {
                    Some(Step::AutoDeref) => self.via.clone(),
                    _ => self.via.wrap(Step::AutoDeref),
                };
                self.expression_via(fact.operand(0), via);
            }
            FactKind::Tuple => {
                // A tuple value is not a reference; autoderef selects it.
                let via = match self.via.steps.first() {
                    Some(Step::AutoDeref) => {
                        self.via.unwrap(Step::AutoDeref).expect("checked step")
                    }
                    _ => self.via.clone(),
                };
                match via.steps.first() {
                    Some(Step::Field(index)) => {
                        let index = *index;
                        let rest = via.unwrap(Step::Field(index)).expect("checked field step");
                        match fact.operand(index as usize) {
                            Some(element) => self.expression_via(Some(element), rest),
                            None => self.defer(node, ConstraintDemand::ExpressionRelation),
                        }
                    }
                    _ => self.defer(node, ConstraintDemand::ExpressionRelation),
                }
            }
            // `return` has Type `!`, which coerces to every Type.
            FactKind::Return => {}
            FactKind::Cast => ready!(self.annotation(db, node)),
            FactKind::Call => {
                if let Some(constructed) =
                    ready!(self.constructor(db, ConstructorSite::Expression(node)))
                {
                    let operands: Vec<(u32, NodeId)> = fact
                        .operands()
                        .iter()
                        .enumerate()
                        .skip(1)
                        .filter_map(|(ordinal, operand)| {
                            Some((u32::try_from(ordinal - 1).ok()?, (*operand)?))
                        })
                        .collect();
                    return self.constructed_inward(db, node, &constructed, &operands);
                }
                match ready!(self.callee(db, node)) {
                    Some(Callee::Source(definition)) => ready!(self.result(db, node, definition)),
                    // The closure's body value is the call's result.
                    Some(Callee::Closure(closure)) => {
                        let body = match self.fact(closure) {
                            Ok(fact) => fact.operand(0),
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        };
                        self.expression(body)
                    }
                    Some(Callee::Facade(LibraryFunction::MustUse)) => {
                        self.expression(fact.operand(1))
                    }
                    // The written result Type of the callable's signature.
                    Some(Callee::Bound(bound)) => ready!(self.bound_result(db, node, &bound)),
                    // `!` coerces to every Type.
                    Some(Callee::Facade(function)) if function.diverges() => {}
                    // `()` or `String`.
                    Some(Callee::Facade(_)) => self.defer(node, ConstraintDemand::NumericFamily),
                    None => {}
                }
            }
            FactKind::MethodCall => ready!(self.method_inward(db, node)),
            FactKind::LangCall => ready!(self.lang_inward(db, node)),
            FactKind::Integer | FactKind::Float => {
                self.defer(node, ConstraintDemand::NumericFamily)
            }
            _ => self.defer(node, ConstraintDemand::ExpressionRelation),
        }
        QueryPoll::Ready(Ok(()))
    }

    /// What each consumer of the expression's value says about its Type.
    fn outward(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        let plan: &'a SourceConstraintPlan = self.plan;
        let uses = plan.uses(node);
        if uses.is_empty() {
            if node == NodeId(0) {
                // The body root's value is the function result.
                let definition = self.key.body.definition;
                ready!(self.result(db, node, definition));
            } else {
                self.mark_unrecorded();
            }
            return QueryPoll::Ready(Ok(()));
        }
        let scalar = self.via.is_scalar();
        for edge in uses {
            let parent = match self.fact(edge.parent) {
                Ok(fact) => fact,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let other = |ordinal: u32| parent.operand(if ordinal == 0 { 1 } else { 0 });
            match parent.kind() {
                FactKind::Parenthesis | FactKind::Block => self.expression(Some(edge.parent)),
                FactKind::Prefix => match parent.operation() {
                    Some(SourceOperator::Negate | SourceOperator::Not) if scalar => {
                        self.expression(Some(edge.parent))
                    }
                    Some(SourceOperator::Deref) => match self.via.steps.first() {
                        Some(Step::Deref) => {
                            let via = self.via.unwrap(Step::Deref).expect("checked step");
                            self.expression_via(Some(edge.parent), via)
                        }
                        // Autoderef of `*e` is autoderef of `e`.
                        Some(Step::AutoDeref) => {
                            let via = self.via.loose();
                            self.expression_via(Some(edge.parent), via)
                        }
                        // `*b` of a `Box<T>` is its content, the box's own
                        // type argument (the builtin dereference); another
                        // Type with an overloaded `Deref` has its `Target`.
                        _ => ready!(self.deref_outward(db, node, edge.parent)),
                    },
                    _ => self.defer(edge.parent, ConstraintDemand::ExpressionRelation),
                },
                FactKind::Binary => match parent.operation() {
                    Some(SourceOperator::Arithmetic) if scalar => {
                        self.expression(Some(edge.parent));
                        self.expression(other(edge.ordinal));
                    }
                    Some(SourceOperator::Shift) if scalar => {
                        if edge.ordinal == 0 {
                            self.expression(Some(edge.parent));
                        }
                    }
                    Some(SourceOperator::Comparison | SourceOperator::CompoundArithmetic)
                        if scalar =>
                    {
                        self.expression(other(edge.ordinal))
                    }
                    // `PartialEq<Self>` (derived or the standard containers'):
                    // both operands hold the same Type at every depth.
                    Some(SourceOperator::Comparison) if self.open => {
                        self.expression(other(edge.ordinal))
                    }
                    // A numeric variable inside an aggregate operand: the same
                    // relation, stated where the operand's Type shows its
                    // comparison impls take `Rhs = Self`.
                    Some(SourceOperator::Comparison) => {
                        ready!(self.aggregate_comparison(
                            db,
                            node,
                            edge.parent,
                            other(edge.ordinal)
                        ))
                    }
                    // `=` gives both places one Type at every depth.
                    Some(SourceOperator::Assign) => self.expression(other(edge.ordinal)),
                    Some(SourceOperator::CompoundShift) => {}
                    _ => self.defer(edge.parent, ConstraintDemand::ExpressionRelation),
                },
                FactKind::Discard => {}
                // A cast constrains its result, not a named operand. Only a
                // literal operand (through parentheses and unary `-`/`!`)
                // takes rustc's castable expectation from the target.
                FactKind::Cast if edge.ordinal == 0 => {
                    if scalar && !self.open && literal_operand(plan, node) {
                        ready!(self.cast_expectation(db, edge.parent));
                    }
                }
                // The initializer's Type is the pattern's matched Type.
                FactKind::Let if edge.ordinal == 1 => {
                    let via = self.via.clone();
                    self.pattern_via(parent.operand(0), via);
                }
                FactKind::Call if edge.ordinal > 0 => {
                    match ready!(self.constructor(db, ConstructorSite::Expression(edge.parent))) {
                        // A constructor argument is that field of the value.
                        Some(constructed) => {
                            match self.field_step(&constructed, edge.ordinal - 1) {
                                Some(step) => {
                                    let via = self.via.wrap(step);
                                    self.expression_via(Some(edge.parent), via);
                                    self.type_argument_relation(
                                        edge.parent,
                                        &constructed,
                                        edge.ordinal - 1,
                                    );
                                }
                                None => {
                                    self.defer(edge.parent, ConstraintDemand::ExpressionRelation)
                                }
                            }
                        }
                        None => ready!(self.argument(db, edge.parent, edge.ordinal)),
                    }
                }
                FactKind::Record => {
                    let rows = self.plan.record_fields(edge.parent).unwrap_or(&[]);
                    match rows.get(edge.ordinal as usize).copied() {
                        Some(row) => {
                            let constructed = ready!(
                                self.constructor(db, ConstructorSite::Expression(edge.parent))
                            );
                            let field = constructed
                                .as_deref()
                                .and_then(|constructed| self.written_field(constructed, row));
                            let step = constructed.as_deref().zip(field).and_then(
                                |(constructed, field)| self.field_step(constructed, field),
                            );
                            match step {
                                Some(step) => {
                                    let via = self.via.wrap(step);
                                    self.expression_via(Some(edge.parent), via);
                                    if let (Some(constructed), Some(field)) =
                                        (constructed.as_deref(), field)
                                    {
                                        self.type_argument_relation(
                                            edge.parent,
                                            constructed,
                                            field,
                                        );
                                    }
                                }
                                None => {
                                    self.defer(edge.parent, ConstraintDemand::ExpressionRelation)
                                }
                            }
                        }
                        // `..base` has the record's own Type.
                        None => self.expression(Some(edge.parent)),
                    }
                }
                FactKind::MethodCall => {
                    ready!(self.method_outward(db, edge.parent, edge.ordinal))
                }
                FactKind::LangCall => ready!(self.lang_outward(db, edge.parent, edge.ordinal)),
                // The iterated head: its `Item` is the pattern's `Some` field.
                FactKind::For if edge.ordinal == 0 => ready!(self.for_head(db, edge.parent)),
                // A loop body's value is `()`.
                FactKind::For => {}
                FactKind::If if edge.ordinal > 0 => {
                    if parent.operand(2).is_some() {
                        self.expression(Some(edge.parent));
                    } else {
                        self.defer(edge.parent, ConstraintDemand::ExpressionRelation);
                    }
                }
                FactKind::Match if edge.ordinal == 0 => {
                    // Every arm's pattern matches the scrutinee's Type.
                    let via = self.via.loose();
                    for pattern in parent.operands().iter().skip(1).step_by(2) {
                        self.pattern_via(*pattern, via.clone());
                    }
                }
                FactKind::Match if edge.ordinal % 2 == 0 => {
                    let via = self.via.loose();
                    self.expression_via(Some(edge.parent), via);
                }
                FactKind::Return => {
                    let definition = self.key.body.definition;
                    ready!(self.result(db, edge.parent, definition));
                }
                // The base of `base.f`.
                FactKind::Field => ready!(self.field_access(db, node, edge.parent)),
                FactKind::Array => {
                    let via = self.via.wrap(Step::Element);
                    self.expression_via(Some(edge.parent), via);
                }
                // The base of `base[index]`: its element is the read. A growable
                // array's element is its type argument.
                FactKind::Index if edge.ordinal == 0 => {
                    if !ready!(self.index_is_element(db, edge.parent)) {
                        self.defer(edge.parent, ConstraintDemand::ExpressionRelation);
                    } else {
                        let steps = self.via.steps.clone();
                        let derefs = steps
                            .iter()
                            .take_while(|step| matches!(step, Step::Deref | Step::AutoDeref))
                            .count();
                        let vector = matches!(steps.get(derefs), Some(Step::TypeArgument(0)))
                            && ready!(self.is_vec_expression(db, node));
                        match steps.get(derefs) {
                            Some(Step::Element) => {
                                let via = Via {
                                    steps: steps[derefs + 1..].to_vec(),
                                    strict: false,
                                };
                                self.expression_via(Some(edge.parent), via);
                            }
                            Some(Step::TypeArgument(0)) if vector => {
                                let via = Via {
                                    steps: steps[derefs + 1..].to_vec(),
                                    strict: false,
                                };
                                self.expression_via(Some(edge.parent), via);
                            }
                            _ => self.defer(edge.parent, ConstraintDemand::ExpressionRelation),
                        }
                    }
                }
                // The base of `base[range]`: its elements are the slice's.
                FactKind::IndexRange if edge.ordinal == 0 => {
                    let steps = self.via.steps.clone();
                    let derefs = steps
                        .iter()
                        .take_while(|step| matches!(step, Step::Deref | Step::AutoDeref))
                        .count();
                    let vector = matches!(steps.get(derefs), Some(Step::TypeArgument(0)))
                        && ready!(self.is_vec_expression(db, node));
                    match steps.get(derefs) {
                        Some(Step::Element) => {
                            let via = Via {
                                steps: steps[derefs..].to_vec(),
                                strict: false,
                            };
                            self.expression_via(Some(edge.parent), via);
                        }
                        Some(Step::TypeArgument(0)) if vector => {
                            let mut rewritten = steps[derefs..].to_vec();
                            rewritten[0] = Step::Element;
                            let via = Via {
                                steps: rewritten,
                                strict: false,
                            };
                            self.expression_via(Some(edge.parent), via);
                        }
                        _ => self.defer(edge.parent, ConstraintDemand::ExpressionRelation),
                    }
                }
                // The index operand: its Type is the selected `Index` impl's.
                FactKind::Index | FactKind::IndexRange if edge.ordinal == 1 => {
                    ready!(self.index_operand(db, edge.parent))
                }
                FactKind::Tuple => {
                    let via = self.via.wrap(Step::Field(edge.ordinal));
                    self.expression_via(Some(edge.parent), via);
                }
                FactKind::Borrow => {
                    let via = self.via.wrap(Step::Deref);
                    self.expression_via(Some(edge.parent), via);
                }
                // An explicit format argument is a generic formatting-trait
                // operand, unless the template may use it as a usize count.
                FactKind::FormatArguments if edge.ordinal > 0 && !parent.counted_format() => {}
                // A closure body's value is what its calls return.
                FactKind::Closure if edge.ordinal == 0 => {
                    ready!(self.closure_calls(db, edge.parent))
                }
                FactKind::Closure => {}
                _ => self.defer(edge.parent, ConstraintDemand::ExpressionRelation),
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// `access` is `base.f` and the current member is `base`. Field access
    /// auto-dereferences its base: only the field `f` of the dereferenced
    /// value is the access's Type, and a variable that is one of the base's
    /// own type arguments is held by `f` where its declared Type names that
    /// parameter. A box is dereferenced to its content, as the member is.
    fn field_access(&mut self, db: &mut Database, base: NodeId, access: NodeId) -> QueryPoll<()> {
        let mut steps = self.via.steps.clone();
        let mut current: Option<TyTerm> = None;
        loop {
            let derefs = steps
                .iter()
                .take_while(|step| matches!(step, Step::Deref | Step::AutoDeref))
                .count();
            let Some(Step::TypeArgument(position)) = steps.get(derefs).copied() else {
                self.select_field(access, &steps, derefs);
                return QueryPoll::Ready(Ok(()));
            };
            let nominal = match current.take() {
                Some(known) => Some(known),
                None => ready!(self.expression_type(db, base)),
            };
            let Some(mut nominal) = nominal else {
                self.defer(access, ConstraintDemand::ExpressionRelation);
                return QueryPoll::Ready(Ok(()));
            };
            while let TyTerm::Ref(_, _, inner) = nominal {
                nominal = *inner;
            }
            // `b.f` of a `Box<P>` is `(*b).f`: the box's own type argument
            // is the content the field is selected from.
            if position == 0
                && let Some(contents) = ready!(self.box_contents(db, &nominal))
            {
                current = Some(contents);
                steps = steps[derefs + 1..].to_vec();
                continue;
            }
            return self.type_argument_field(
                db,
                access,
                nominal,
                position,
                steps[derefs + 1..].to_vec(),
            );
        }
    }
    /// The access `base.f` of a dereferenced base: `steps[derefs]` is the
    /// projection of the base's field.
    fn select_field(&mut self, access: NodeId, steps: &[Step], derefs: usize) {
        let parent = match self.fact(access) {
            Ok(fact) => fact,
            Err(_) => return,
        };
        let selected = match (parent.field(), parent.member(), steps.get(derefs)) {
            (Some(position), _, Some(Step::Field(selected))) => Some(*selected == position),
            (None, Some(row), Some(Step::Named(selected))) => Some(*selected == row),
            _ => None,
        };
        match selected {
            Some(true) => {
                let via = Via {
                    steps: steps[derefs + 1..].to_vec(),
                    strict: false,
                };
                self.expression_via(Some(access), via);
            }
            // Another field of the same value.
            Some(false) => {}
            None => self.defer(access, ConstraintDemand::ExpressionRelation),
        }
    }

    /// A `let` or match-arm root pattern matched against a value whose Type,
    /// projected by the current member's steps, is the variable: what the
    /// pattern engine's retained tree says about that Type, and which value
    /// the root matches.
    fn pattern(&mut self, db: &mut Database, root: NodeId) -> QueryPoll<()> {
        let plan: &'a SourceConstraintPlan = self.plan;
        match plan.tree(root) {
            Some(tree) => {
                let via = self.via.clone();
                let outer = self.root_via.replace(via.clone());
                let walked = self.walk(db, root, tree.root(), via);
                self.root_via = outer;
                ready!(walked);
            }
            None => self.defer(root, ConstraintDemand::Binding),
        }
        // A pattern function's parameter pattern: the current member is a
        // part of the declared Type of the argument it matches.
        if let Some(ordinal) = plan.fact(root).and_then(|fact| fact.parameter()) {
            let definition = self.key.body.definition;
            if let Some(term) = ready!(self.parameter_term(db, root, definition, ordinal)) {
                ready!(self.constrain_term(db, root, &term, ConstraintDemand::ParameterType));
            }
            return QueryPoll::Ready(Ok(()));
        }
        let uses = plan.uses(root);
        if uses.is_empty() {
            self.defer(root, ConstraintDemand::Binding);
        }
        for edge in uses {
            let parent = match self.fact(edge.parent) {
                Ok(fact) => fact,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            match parent.kind() {
                FactKind::Let if edge.ordinal == 0 => {
                    if parent.annotation().is_some() {
                        ready!(self.annotation(db, edge.parent));
                    }
                    self.expression(parent.operand(1));
                }
                FactKind::Match if edge.ordinal % 2 == 1 => {
                    let via = self.via.loose();
                    self.expression_via(parent.operand(0), via.clone());
                    for pattern in parent.operands().iter().skip(1).step_by(2) {
                        self.pattern_via(*pattern, via.clone());
                    }
                }
                FactKind::For if edge.ordinal == 1 => {
                    ready!(self.for_pattern(db, root, edge.parent))
                }
                // A closure parameter pattern: the argument's written Type,
                // else the `Fn*(A, ..)` bound the closure is passed for.
                FactKind::Closure if edge.ordinal >= 1 => {
                    if self
                        .plan
                        .fact(root)
                        .is_some_and(|fact| fact.annotation_node().is_some())
                    {
                        ready!(self.annotation(db, root));
                    } else {
                        ready!(self.closure_parameter_bound(db, root));
                    }
                }
                _ => self.defer(root, ConstraintDemand::Binding),
            }
        }
        QueryPoll::Ready(Ok(()))
    }
    fn walk(
        &mut self,
        db: &mut Database,
        root: NodeId,
        node: &PatternNode,
        via: Via,
    ) -> QueryPoll<()> {
        let current = std::mem::replace(&mut self.via, via);
        let result = self.walk_current(db, root, node);
        self.via = current;
        result
    }
    /// One retained pattern node against the current member's projection.
    fn walk_current(
        &mut self,
        db: &mut Database,
        root: NodeId,
        node: &PatternNode,
    ) -> QueryPoll<()> {
        let peeled = self.peeled;
        let result = self.walk_node(db, root, node);
        self.peeled = peeled;
        result
    }
    fn walk_node(&mut self, db: &mut Database, root: NodeId, node: &PatternNode) -> QueryPoll<()> {
        // Default binding modes: a pattern that is not a reference pattern
        // or a binding, matched against a reference, matches the referent
        // and binds its names by reference.
        if matches!(
            node,
            PatternNode::Tuple { .. }
                | PatternNode::Array { .. }
                | PatternNode::Constructor { .. }
                | PatternNode::Literal(_)
                | PatternNode::Range { .. }
        ) {
            while let Some(via) = self.via.unwrap(Step::Deref) {
                self.via = via;
                self.peeled = true;
            }
        }
        match node {
            PatternNode::Wildcard => {}
            PatternNode::Binding {
                occurrence,
                binding,
                by_reference,
                mutable,
                subpattern,
                ..
            } => {
                let by_reference = &(*by_reference || (self.peeled && !*mutable));
                // Or-alternatives share the canonical binding's identity.
                match ready!(self.pattern_binding(db, *binding)) {
                    Some(identity) if *by_reference => {
                        let via = self.via.wrap(Step::Deref);
                        let current = std::mem::replace(&mut self.via, via);
                        self.binding(identity, *binding);
                        self.via = current;
                    }
                    Some(identity) => self.binding(identity, *binding),
                    // An identifier naming a constant or a unit constructor
                    // matches by value: a constant states its declared Type.
                    None => {
                        let observed = ready!(db.fetch(
                            &SyntaxProvider,
                            QueryKey::EnteredValue(entry(self.key, *binding))
                        ));
                        let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
                            return QueryPoll::Ready(Err(fault(
                                "pattern name query variant differs",
                            )));
                        };
                        match observed.declaration() {
                            Some(declaration)
                                if !selection_pending(observed)
                                    && observed.local_binding().is_none() =>
                            {
                                let identity = declaration.identity();
                                if let Some(term) =
                                    ready!(self.declared_item_type(db, observed, identity))
                                {
                                    ready!(self.constrain_term(
                                        db,
                                        *occurrence,
                                        &term,
                                        ConstraintDemand::SelectedDeclaration
                                    ));
                                }
                            }
                            _ => self.defer(*occurrence, ConstraintDemand::Binding),
                        }
                    }
                }
                if let Some(subpattern) = subpattern {
                    let via = self.via.clone();
                    ready!(self.walk(db, root, subpattern, via));
                }
            }
            PatternNode::Literal(literal) => {
                if let Err(error) = self.pattern_literal(root, literal) {
                    return QueryPoll::Ready(Err(error));
                }
            }
            PatternNode::Range { start, end, .. } => {
                for endpoint in [start, end].into_iter().flatten() {
                    match endpoint {
                        Ok(RangeEndpoint::Literal(literal)) => {
                            if let Err(error) = self.pattern_literal(root, literal) {
                                return QueryPoll::Ready(Err(error));
                            }
                        }
                        // A constant bound's Type is its declaration's; the
                        // service draws no conclusion from it.
                        Ok(RangeEndpoint::Constant(_)) | Err(_) => self.mark_unrecorded(),
                    }
                }
            }
            PatternNode::Tuple { items, rest } => match self.via.steps.first() {
                Some(Step::Field(ordinal)) => {
                    let ordinal = *ordinal;
                    let via = self
                        .via
                        .unwrap(Step::Field(ordinal))
                        .expect("checked field step");
                    if let Some(rest) = rest.filter(|rest| ordinal >= *rest) {
                        // A position after `..` needs the matched arity.
                        match ready!(self.tuple_arity(db, root)) {
                            Some(arity) => {
                                let (items_len, rest) = (items.len(), rest as usize);
                                let trailing = items_len - rest.min(items_len);
                                let position = ordinal as usize;
                                // The trailing patterns match the last positions; the
                                // positions between the leading ones and those are `..`.
                                if arity >= items_len && position + trailing >= arity {
                                    if let Some(item) = items.get(position + items_len - arity) {
                                        ready!(self.walk(db, root, item, via));
                                    } else {
                                        self.defer(root, ConstraintDemand::Binding);
                                    }
                                }
                            }
                            None => self.mark_unrecorded(),
                        }
                    } else if let Some(item) = items.get(ordinal as usize) {
                        ready!(self.walk(db, root, item, via));
                    } else {
                        self.defer(root, ConstraintDemand::Binding);
                    }
                }
                // A tuple pattern against a reference adjusts binding modes,
                // which needs the shared pattern Type relation.
                _ => self.defer(root, ConstraintDemand::Binding),
            },
            PatternNode::Reference { inner, .. } => match self.via.unwrap(Step::Deref) {
                Some(via) => {
                    // A reference pattern resets the default binding mode.
                    self.peeled = false;
                    ready!(self.walk(db, root, inner, via))
                }
                None => self.defer(root, ConstraintDemand::Binding),
            },
            PatternNode::Or(alternatives) => {
                for alternative in alternatives {
                    let via = self.via.clone();
                    ready!(self.walk(db, root, alternative, via));
                }
            }
            // Every element pattern matches the array/slice element Type.
            PatternNode::Array { items, .. } => match self.via.unwrap(Step::Element) {
                Some(via) => {
                    for item in items {
                        ready!(self.walk(db, root, item, via.clone()));
                    }
                }
                None => self.defer(root, ConstraintDemand::Binding),
            },
            // The field pattern of the declared field the projection selects.
            PatternNode::Constructor { slot, form } => {
                ready!(self.walk_constructor(db, root, *slot, form))
            }
            PatternNode::Unsupported(_) => self.mark_unrecorded(),
        }
        QueryPoll::Ready(Ok(()))
    }
    /// A literal pattern has the matched Type: its suffix is concrete, an
    /// unsuffixed literal only fixes the numeric family.
    fn pattern_literal(
        &mut self,
        root: NodeId,
        literal: &PatternLiteral,
    ) -> Result<(), Diagnostic> {
        if !self.via.is_scalar() {
            self.defer(root, ConstraintDemand::Binding);
            return Ok(());
        }
        match literal {
            PatternLiteral::Integer { suffix, .. } | PatternLiteral::Float { suffix, .. } => {
                let floating = matches!(literal, PatternLiteral::Float { .. });
                match suffix {
                    Some(primitive) => self.constrain(root, *primitive)?,
                    None if self.open => self.literal_reached(floating),
                    None if floating != self.floating => {
                        self.defer(root, ConstraintDemand::NumericFamily)
                    }
                    None => {}
                }
            }
            _ => self.defer(root, ConstraintDemand::NumericFamily),
        }
        Ok(())
    }

    fn binding_pattern(
        &mut self,
        db: &mut Database,
        use_node: NodeId,
        identity: Digest,
    ) -> QueryPoll<Option<NodeId>> {
        binding_pattern(db, self.key, self.plan, use_node, identity)
    }

    /// The binding's own pattern (its declaration) and every actual use.
    fn declared(&mut self, db: &mut Database, identity: Digest, pattern: NodeId) -> QueryPoll<()> {
        let plan: &'a SourceConstraintPlan = self.plan;
        // The value this binding's root pattern matches, at its position.
        match plan.occurrence(pattern) {
            Some(occurrence) => {
                let (matched, skipped) = ready!(self.root_matched_type(db, occurrence.root));
                match ready!(self.occurrence_steps(db, occurrence, skipped, matched)) {
                    Some((steps, by_reference)) => {
                        // A reference binding's projection starts at its
                        // referent (a field access autoderefs it).
                        let bound = if by_reference {
                            self.via
                                .unwrap(Step::Deref)
                                .or_else(|| self.via.unwrap(Step::AutoDeref))
                        } else {
                            Some(self.via.clone())
                        };
                        match bound {
                            Some(bound) => {
                                let via = bound.within(&steps);
                                self.pattern_via(Some(occurrence.root), via);
                            }
                            None => self.defer(pattern, ConstraintDemand::Binding),
                        }
                    }
                    None => self.defer(pattern, ConstraintDemand::Binding),
                }
            }
            // A closure parameter has its written Type, if any; without one
            // its call argument decides it, which no source use here sees.
            None if plan
                .fact(pattern)
                .is_some_and(|fact| fact.closure_parameter()) =>
            {
                if plan
                    .fact(pattern)
                    .is_some_and(|fact| fact.annotation_node().is_some())
                {
                    ready!(self.annotation(db, pattern));
                } else {
                    // Passed for a parameter bounded by `Fn*(A, ..) -> R`: the
                    // parameter holds the bound's `A`.
                    ready!(self.closure_parameter_bound(db, pattern));
                }
            }
            // A plain parameter of a pattern function (E-R5-010): its
            // declared Type through the common parameter goal.
            None if plan
                .fact(pattern)
                .is_some_and(|fact| fact.parameter().is_some()) =>
            {
                let definition = self.key.body.definition;
                if let Some(ordinal) = plan.fact(pattern).and_then(|fact| fact.parameter())
                    && let Some(term) =
                        ready!(self.parameter_term(db, pattern, definition, ordinal))
                {
                    ready!(self.constrain_term(
                        db,
                        pattern,
                        &term,
                        ConstraintDemand::ParameterType
                    ));
                }
            }
            // A parameter, loop or `if let` binding: its Type needs the
            // shared pattern relation of that owner.
            None => self.defer(pattern, ConstraintDemand::Binding),
        }
        let Some((_, row)) = plan.name_of(pattern) else {
            self.defer(pattern, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(()));
        };
        // An unindexed region after the declaration may read this local. A
        // pattern function's parameter patterns are indexed after its block
        // (E-R5-010) but are declared before every region of the body.
        let parameter = plan
            .fact(pattern)
            .is_some_and(|fact| fact.parameter().is_some());
        if row
            .hidden()
            .iter()
            .any(|region| parameter || *region > pattern)
        {
            self.mark_unrecorded();
        }
        for use_node in row.paths().iter().copied() {
            let observed = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::EnteredValue(entry(self.key, use_node))
            ));
            let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("binding use query variant differs")));
            };
            let local = selected_local(observed);
            if selection_pending(observed) && local.is_none() {
                self.defer(use_node, ConstraintDemand::ValueUse);
                continue;
            }
            if local == Some(identity) {
                self.expression(Some(use_node));
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// Whether the index expression `index` (`base[i]`) reads one element
    /// place: a literal index certainly does; another index operand does
    /// unless its static Type is a nominal (a `Range` value slices).
    pub(super) fn index_is_element(&mut self, db: &mut Database, index: NodeId) -> QueryPoll<bool> {
        let fact = match self.fact(index) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if fact.flag() {
            return QueryPoll::Ready(Ok(true));
        }
        let Some(operand) = fact.operand(1) else {
            return QueryPoll::Ready(Ok(false));
        };
        QueryPoll::Ready(Ok(!matches!(
            ready!(self.expression_type(db, operand)),
            Some(TyTerm::Nominal(..))
        )))
    }
    /// Whether the static Type of `node`, through references, is the
    /// growable array lang item.
    pub(super) fn is_vec_expression(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<bool> {
        let Some(mut term) = ready!(self.expression_type(db, node)) else {
            return QueryPoll::Ready(Ok(false));
        };
        while let TyTerm::Ref(_, _, inner) = term {
            term = *inner;
        }
        QueryPoll::Ready(Ok(ready!(self.vec_elements(db, &term)).is_some()))
    }

    fn solve(&mut self, db: &mut Database) -> QueryPoll<()> {
        loop {
            if let Some((identity, pattern, via)) = self.patterns.pop() {
                self.via = via;
                ready!(self.declared(db, identity, pattern));
            } else if let Some((node, via)) = self.pattern_work.pop() {
                self.via = via;
                ready!(self.pattern(db, node));
            } else if let Some((node, via)) = self.work.pop() {
                self.via = via;
                ready!(self.inward(db, node));
                ready!(self.outward(db, node));
            } else {
                return QueryPoll::Ready(Ok(()));
            }
        }
    }
    /// Indexed patterns of the scalar locals (every one has the variable's Type).
    fn members(&self) -> Vec<NodeId> {
        let mut members: Vec<NodeId> = self
            .bindings
            .iter()
            .filter(|((_, via), _)| via.is_scalar())
            .map(|(_, pattern)| *pattern)
            .collect();
        members.sort_unstable();
        members.dedup();
        members
    }
}

/// The local binding an indexed identifier pattern introduces, through the
/// ordinary hygienic Value goal.
fn pattern_binding(db: &mut Database, key: &PcKey, pattern: NodeId) -> QueryPoll<Option<Digest>> {
    let selected = ready!(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(entry(key, pattern))));
    let QueryValue::EnteredValue(target) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding target query variant differs")));
    };
    let goal = target;
    let (Some(binding), Some(target)) = (goal.local_binding(), goal.binding_target()) else {
        // The pattern's module classification is not published yet: its
        // retained occurrence row still states the binding's identity.
        return patterns::unclassified_binding(db, key, pattern, goal);
    };
    if binding != target.identity() {
        return QueryPoll::Ready(Err(fault("binding target occurrence identity differs")));
    }
    QueryPoll::Ready(Ok(Some(binding)))
}
/// Select the indexed pattern of a local selected for a path use. Equal
/// spelling is only a candidate filter; the actual hygienic binding identity
/// of the candidate pattern must match.
fn binding_pattern(
    db: &mut Database,
    key: &PcKey,
    plan: &SourceConstraintPlan,
    use_node: NodeId,
    identity: Digest,
) -> QueryPoll<Option<NodeId>> {
    let Some((_, row)) = plan.name_of(use_node) else {
        return QueryPoll::Ready(Ok(None));
    };
    for candidate in row.bindings() {
        if ready!(pattern_binding(db, key, *candidate)) == Some(identity) {
            return QueryPoll::Ready(Ok(Some(*candidate)));
        }
    }
    QueryPoll::Ready(Ok(None))
}
/// The declaring `let` statement of the local a path expression reads, when
/// that local is a plain `let` binding. Selection uses the ordinary hygienic
/// Value goals; this never lowers or executes the statement.
pub(crate) fn declaring_let(db: &mut Database, key: &PcKey) -> QueryPoll<Option<PcKey>> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("declaring let goal requires entry phase")));
    }
    let facts = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(key.body.clone())
    ));
    let QueryValue::SourceConstraints(plan) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint source plan variant differs")));
    };
    let node = NodeId(key.node);
    if plan.fact(node).map(|fact| fact.kind()) != Some(FactKind::Path) {
        return QueryPoll::Ready(Ok(None));
    }
    let observed = ready!(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key.clone())));
    let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding use query variant differs")));
    };
    let Some(identity) = selected_local(observed) else {
        return QueryPoll::Ready(Ok(None));
    };
    let Some(pattern) = ready!(binding_pattern(db, key, plan, node, identity)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let statement = plan
        .uses(pattern)
        .iter()
        .find(|edge| {
            edge.ordinal == 0
                && plan.fact(edge.parent).map(|fact| fact.kind()) == Some(FactKind::Let)
        })
        .map(|edge| entry(key, edge.parent));
    if statement.is_some() {
        return QueryPoll::Ready(Ok(statement));
    }
    // A binding destructured from a `let` (`let (mut a, b) = (1, 2.0);`):
    // the goal of the literal its position selects in the initializer,
    // whose component holds this binding.
    QueryPoll::Ready(Ok(
        destructured_literal(plan, pattern).map(|literal| entry(key, literal))
    ))
}
/// The unsuffixed literal a nested `let` binding's position selects in its
/// initializer's written tuple/array/borrow structure.
fn destructured_literal(plan: &SourceConstraintPlan, pattern: NodeId) -> Option<NodeId> {
    let occurrence = plan.occurrence(pattern)?;
    if occurrence.by_reference || occurrence.steps.is_empty() {
        return None;
    }
    let statement = plan.uses(occurrence.root).iter().find(|edge| {
        edge.ordinal == 0 && plan.fact(edge.parent).map(|fact| fact.kind()) == Some(FactKind::Let)
    })?;
    let mut node = plan.fact(statement.parent)?.operand(1)?;
    for step in &occurrence.steps {
        // Parentheses keep their operand's value.
        while let Some(fact) = plan.fact(node)
            && fact.kind() == FactKind::Parenthesis
        {
            node = fact.operand(0)?;
        }
        let fact = plan.fact(node)?;
        node = match (step, fact.kind()) {
            (PatternStep::Field(position), FactKind::Tuple) => fact.operand(*position as usize)?,
            (PatternStep::Element, FactKind::Array) => fact.operand(0)?,
            (PatternStep::Deref, FactKind::Borrow) => fact.operand(0)?,
            _ => return None,
        };
    }
    // Through parentheses and unary `-`/`!` to the literal itself.
    loop {
        let fact = plan.fact(node)?;
        match fact.kind() {
            FactKind::Integer | FactKind::Float => return Some(node),
            FactKind::Parenthesis => node = fact.operand(0)?,
            FactKind::Prefix if fact.numeric_prefix() => node = fact.operand(0)?,
            _ => return None,
        }
    }
}

/// Pure provider. Syntax discovery/prelude publication must already be genuine;
/// missing authority remains pending and never becomes a guessed primitive.
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedBindingConstraints> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("binding constraint goal requires entry phase")));
    }
    let facts = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(key.body.clone())
    ));
    let QueryValue::SourceConstraints(plan) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint source plan variant differs")));
    };
    let mut result = PreparedBindingConstraints {
        key: key.clone(),
        structure: plan.structure(),
        binding: None,
        members: Vec::new(),
        state: BindingConstraintState::NoNumericDemand,
        written: None,
        arguments: None,
        related_numeric: None,
    };
    let Some(fact) = plan.fact(NodeId(key.node)) else {
        return QueryPoll::Ready(Err(fault("binding constraint node absent")));
    };
    // The goal's own inference variable: a plain `let` binding with a bare
    // numeric literal initializer, or a bare unsuffixed literal expression.
    let (floating, start) = match fact.kind() {
        FactKind::Let if fact.annotation().is_none() => {
            let [Some(pattern), Some(initializer)] = fact.operands() else {
                return QueryPoll::Ready(Ok(result));
            };
            let floating = match numeric_initializer(plan, *initializer) {
                Ok(Some(floating)) => floating,
                Ok(None) => return QueryPoll::Ready(Ok(result)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let selected = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::EnteredValue(entry(key, *pattern))
            ));
            let QueryValue::EnteredValue(target) = selected.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("binding target query variant differs")));
            };
            let Some(binding) = target.local_binding() else {
                return result.pending(*pattern, ConstraintDemand::Binding);
            };
            let Some(target) = target.binding_target() else {
                return result.pending(*pattern, ConstraintDemand::Binding);
            };
            if binding != target.identity() {
                return QueryPoll::Ready(Err(fault("binding target occurrence identity differs")));
            }
            result.binding = Some(binding);
            (floating, Start::Binding(binding, *pattern))
        }
        FactKind::Integer => (false, Start::Literal(NodeId(key.node))),
        FactKind::Float => (true, Start::Literal(NodeId(key.node))),
        // A call or fieldless constructor whose type arguments no argument
        // fixes: later code decides them.
        FactKind::Path | FactKind::Call | FactKind::MethodCall | FactKind::Record => {
            return open::prepare_expression(db, key, plan, result);
        }
        _ => return QueryPoll::Ready(Ok(result)),
    };
    let mut component = Component::new(key, plan, floating);
    component.literal_start = matches!(start, Start::Literal(_));
    match start {
        Start::Binding(binding, pattern) => component.binding(binding, pattern),
        Start::Literal(node) => {
            // A nested demand for this literal's Type (see `decided_numeric`)
            // is not asked again while it is being decided.
            component.deciding.push(node);
            component.expression(Some(node))
        }
    }
    ready!(component.solve(db));
    // In well-typed source every relation agrees, so one concrete primitive
    // is the variable's Type even beside relations that remain pending.
    result.state = match (component.concrete, component.pending) {
        (Some((_, primitive)), _) => BindingConstraintState::Resolved(primitive),
        (None, Some((node, demand))) => BindingConstraintState::Pending { node, demand },
        (None, None) if component.unrecorded => BindingConstraintState::Unconstrained,
        // A closed component: Rust's integer/float fallback.
        (None, None) => BindingConstraintState::Resolved(if floating {
            PrimitiveType::F64
        } else {
            PrimitiveType::I32
        }),
    };
    if matches!(result.state, BindingConstraintState::Resolved(_)) {
        result.members = component.members();
    } else if component.literal_start {
        result.related_numeric = component.related_numeric_plan();
    }
    QueryPoll::Ready(Ok(result))
}
enum Start {
    Binding(Digest, NodeId),
    Literal(NodeId),
}
impl StableEncode for PreparedBindingConstraints {
    const DOMAIN: &'static str = "lrr/v2/prepared-binding-constraints";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.structure);
        e.option(&self.binding, |e, id| e.digest(*id));
        e.sequence(&self.members, |e, node| e.u32(node.0));
        e.option(&self.related_numeric, |e, plan| e.record(plan.as_ref()));
        e.option(&self.written, |e, written| e.record(written.as_ref()));
        e.option(&self.arguments, |e, inferred| {
            e.digest(inferred.definition);
            e.sequence(&inferred.arguments, |e, (ordinal, written)| {
                e.u32(*ordinal);
                e.record(written.as_ref());
            });
        });
        match self.state {
            BindingConstraintState::NoNumericDemand => e.u8(0),
            BindingConstraintState::Unconstrained => e.u8(1),
            BindingConstraintState::Resolved(primitive) => {
                e.u8(2);
                e.u32(primitive.tag());
            }
            BindingConstraintState::ResolvedType => e.u8(4),
            BindingConstraintState::Pending { node, demand } => {
                e.u8(3);
                e.u32(node.0);
                e.u8(demand.tag());
            }
        }
    }
}
