//! Source-component rows: the retained, unsolved relations of one numeric
//! literal's connected source component, produced from the shared
//! `SourceRelationPlan` adjacency and the ordinary declaration/value/pattern
//! services. Rows are Type equations over source-owned variables plus typed
//! `Hole`s; nothing here decides a primitive, selects a candidate or applies a
//! fallback. The pinned inference owner (`traits::upstream::poll_component_root`)
//! solves the rows; the legacy `Component` consumer is untouched.
//!
//! A relation family the producer does not state is an explicit `Hole`, never
//! silence: a component with a hole cannot prove its variable unconstrained, so
//! no fallback may be applied to it.
use super::{Component, binding_pattern, entry, fault, pattern_binding, selected_local, selection_pending};
use crate::{
    core::digest::Digest,
    db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    ir::{
        op::PrimitiveType,
        pc::{NodeId, PcKey},
    },
    syntax::{
        pattern_tree::{PatternLiteral, PatternNode},
        signatures::GenericKind,
        source_constraint_relations::SourceRelationConsumer,
        source_constraints::{FactKind, SourceConstraintPlan, SourceOperator, SourceUse},
    },
    traits::term::{GenericArg, ParamKey, RegionTerm, TyTerm},
};
use std::collections::{BTreeMap, BTreeSet};

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RowFamily {
    Type,
    Integer,
    Float,
}

/// A source Type in terms of the component's variables.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum RowTerm {
    Var(u32),
    /// A closed (or callee-parameter-free) declared source Type.
    Closed(TyTerm),
    Tuple(Vec<RowTerm>),
    Ref { mutable: bool, inner: Box<RowTerm> },
}

/// One equation the pinned owner relates; `node` is its source occurrence.
#[derive(Debug, Clone)]
pub(crate) struct RowEq {
    pub(crate) node: NodeId,
    pub(crate) left: RowTerm,
    pub(crate) right: RowTerm,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BinaryRowKind {
    Arithmetic,
    Comparison,
}

/// Structural relations whose form depends on a type the owner has not
/// resolved yet (rustc's `structurally_resolve_type` points). They fire after
/// the immediate rows are solved.
#[derive(Debug, Clone)]
pub(crate) enum DeferredRow {
    /// `*operand`, builtin deref of a reference: the reference mutability is
    /// read from the resolved operand type.
    Deref {
        node: NodeId,
        operand: RowTerm,
        result: RowTerm,
    },
    /// A builtin binary operator when both operand types are numeric.
    Binary {
        node: NodeId,
        kind: BinaryRowKind,
        lhs: RowTerm,
        rhs: RowTerm,
        result: RowTerm,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RowHole {
    pub(crate) node: NodeId,
    pub(crate) why: String,
}

/// One selected generic function call: its generic Type parameters have one
/// variable each, shared by every relation of this site.
#[derive(Debug, Clone)]
pub(crate) struct RowSite {
    pub(crate) node: NodeId,
    pub(crate) callee: Digest,
    pub(crate) generics: Vec<(ParamKey, u32)>,
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentRows {
    pub(crate) key: PcKey,
    pub(crate) structure: Digest,
    /// The scope of this component's source variables (`InferVarKey`).
    pub(crate) transaction: Digest,
    pub(crate) leaf: u32,
    pub(crate) vars: Vec<RowFamily>,
    /// Unsuffixed numeric literals of the component and their variables.
    pub(crate) literals: Vec<(NodeId, u32)>,
    pub(crate) equations: Vec<RowEq>,
    pub(crate) deferred: Vec<DeferredRow>,
    pub(crate) sites: Vec<RowSite>,
    pub(crate) holes: Vec<RowHole>,
}

struct Builder<'a> {
    c: Component<'a>,
    plan: &'a SourceConstraintPlan,
    rows: ComponentRows,
    node_var: BTreeMap<NodeId, u32>,
    local_var: BTreeMap<Digest, u32>,
    touched: BTreeSet<NodeId>,
    sites_done: BTreeSet<NodeId>,
    groups_done: BTreeSet<NodeId>,
    enrolled: BTreeSet<Digest>,
    work: Vec<NodeId>,
}

fn region_free(term: &TyTerm) -> TyTerm {
    fn arg(arg: &GenericArg) -> GenericArg {
        match arg {
            GenericArg::Type(ty) => GenericArg::Type(region_free(ty)),
            GenericArg::Region(_) => GenericArg::Region(RegionTerm::Static),
            other => other.clone(),
        }
    }
    match term {
        TyTerm::Ref(_, mutable, inner) => TyTerm::Ref(RegionTerm::Static, *mutable, Box::new(region_free(inner))),
        TyTerm::Slice(inner) => TyTerm::Slice(Box::new(region_free(inner))),
        TyTerm::Array(inner, len) => TyTerm::Array(Box::new(region_free(inner)), len.clone()),
        TyTerm::Tuple(members) => TyTerm::Tuple(members.iter().map(region_free).collect()),
        TyTerm::Nominal(id, args) => TyTerm::Nominal(*id, args.iter().map(arg).collect()),
        other => other.clone(),
    }
}

impl<'a> Builder<'a> {
    fn var_key(&self, var: u32) -> crate::traits::term::InferVarKey {
        crate::traits::term::InferVarKey {
            transaction: self.rows.transaction,
            ordinal: var,
        }
    }
    fn hole(&mut self, node: NodeId, why: impl Into<String>) {
        let hole = RowHole { node, why: why.into() };
        if !self.rows.holes.contains(&hole) {
            self.rows.holes.push(hole);
        }
    }
    fn fresh(&mut self, family: RowFamily) -> u32 {
        self.rows.vars.push(family);
        (self.rows.vars.len() - 1) as u32
    }
    fn touch(&mut self, node: NodeId) {
        if self.touched.insert(node) {
            self.work.push(node);
        }
    }
    fn equate(&mut self, node: NodeId, left: RowTerm, right: RowTerm) {
        self.rows.equations.push(RowEq { node, left, right });
    }
    /// The Type of an expression node: a suffixed literal is its primitive,
    /// every other node has its own variable.
    fn term(&mut self, node: NodeId) -> RowTerm {
        if let Some(var) = self.node_var.get(&node) {
            return RowTerm::Var(*var);
        }
        let kind = self.plan.fact(node).map(|fact| (fact.kind(), fact.suffix()));
        match kind {
            Some((FactKind::SuffixedInteger | FactKind::SuffixedFloat, Some(primitive))) => {
                return RowTerm::Closed(TyTerm::Primitive(primitive));
            }
            _ => {}
        }
        let family = match kind {
            Some((FactKind::Integer, _)) => RowFamily::Integer,
            Some((FactKind::Float, _)) => RowFamily::Float,
            _ => RowFamily::Type,
        };
        let var = self.fresh(family);
        self.node_var.insert(node, var);
        if family != RowFamily::Type {
            self.rows.literals.push((node, var));
        }
        RowTerm::Var(var)
    }
    fn local(&mut self, identity: Digest) -> RowTerm {
        if let Some(var) = self.local_var.get(&identity) {
            return RowTerm::Var(*var);
        }
        let var = self.fresh(RowFamily::Type);
        self.local_var.insert(identity, var);
        RowTerm::Var(var)
    }

    /// Run a Component service and report whether it deferred (a missing
    /// goal): the service's own deferral is a hole here.
    fn guarded<T>(
        &mut self,
        f: impl FnOnce(&mut Component<'a>) -> QueryPoll<T>,
    ) -> QueryPoll<(T, bool)> {
        let saved = (self.c.unrecorded, self.c.pending);
        self.c.unrecorded = false;
        self.c.pending = None;
        let outcome = f(&mut self.c);
        let deferred = self.c.unrecorded || self.c.pending.is_some();
        self.c.unrecorded = saved.0;
        self.c.pending = saved.1;
        QueryPoll::Ready(Ok((ready!(outcome), deferred)))
    }

    fn process(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        let plan = self.plan;
        let Some(fact) = plan.fact(node) else {
            return QueryPoll::Ready(Err(fault("component row node absent")));
        };
        let kind = fact.kind();
        ready!(self.inward(db, node, kind));
        let Some(uses) = plan.relations().outward(node) else {
            return QueryPoll::Ready(Err(fault("component row relation node absent")));
        };
        if uses.is_empty() {
            if node == NodeId(0) {
                // The body root's value is the function result.
                let definition = self.rows.key.body.definition;
                let (result, deferred) = ready!(self.guarded(|c| c.result_term(db, node, definition)));
                let own = self.term(node);
                match result.filter(|term| !deferred && crate::frontend::call_contracts::closed_term(term)) {
                    Some(term) => self.equate(node, own, RowTerm::Closed(region_free(&term))),
                    None => self.hole(node, "function result type"),
                }
            } else {
                // A value consumed by syntax the source plan does not record.
                self.hole(node, "unrecorded consumer");
            }
            return QueryPoll::Ready(Ok(()));
        }
        for relation in uses.to_vec() {
            ready!(self.outward(db, node, relation.edge, relation.consumer));
        }
        QueryPoll::Ready(Ok(()))
    }

    /// The node's own relations with its operands.
    fn inward(&mut self, db: &mut Database, node: NodeId, kind: FactKind) -> QueryPoll<()> {
        let plan = self.plan;
        let fact = plan.fact(node).expect("checked fact");
        let own = self.term(node);
        match kind {
            FactKind::Integer | FactKind::Float | FactKind::SuffixedInteger | FactKind::SuffixedFloat => {
                if matches!(kind, FactKind::SuffixedInteger | FactKind::SuffixedFloat) && fact.suffix().is_none() {
                    self.hole(node, "unknown literal suffix");
                }
            }
            FactKind::Parenthesis | FactKind::Block => match fact.operand(0) {
                Some(inner) => {
                    let inner_term = self.term(inner);
                    self.equate(node, own, inner_term);
                    self.touch(inner);
                }
                // A block with no tail expression is `()` or diverges; a
                // diverging block's `!` coerces to every Type, so it states
                // nothing here. (A `()` block is not a numeric component.)
                None => {}
            },
            FactKind::If => {
                // The condition's own `bool` Type is stated where the condition
                // is a member (`outward`); it says nothing about the value.
                for branch in [fact.operand(1), fact.operand(2)].into_iter().flatten() {
                    let branch_term = self.term(branch);
                    self.equate(node, own.clone(), branch_term);
                    self.touch(branch);
                }
                if fact.operand(2).is_none() {
                    self.hole(node, "if without else");
                }
            }
            FactKind::Match => {
                for body in fact.operands().iter().skip(2).step_by(2).flatten() {
                    let body_term = self.term(*body);
                    self.equate(node, own.clone(), body_term);
                    self.touch(*body);
                }
            }
            FactKind::Borrow => match fact.operand(0) {
                Some(inner) => {
                    let inner_term = self.term(inner);
                    self.equate(
                        node,
                        own,
                        RowTerm::Ref {
                            mutable: fact.flag(),
                            inner: Box::new(inner_term),
                        },
                    );
                    self.touch(inner);
                }
                None => self.hole(node, "borrow without operand"),
            },
            // A raw pointer to the operand's place: the operand is touched;
            // the pointer's own row stays unconstrained.
            FactKind::RawBorrow => match fact.operand(0) {
                Some(inner) => self.touch(inner),
                None => self.hole(node, "raw borrow without operand"),
            },
            FactKind::Tuple => {
                let mut members = Vec::new();
                for element in fact.operands() {
                    match element {
                        Some(element) => {
                            members.push(self.term(*element));
                            self.touch(*element);
                        }
                        None => {
                            self.hole(node, "tuple with a missing element");
                            return QueryPoll::Ready(Ok(()));
                        }
                    }
                }
                self.equate(node, own, RowTerm::Tuple(members));
            }
            FactKind::Prefix => match (fact.operation(), fact.operand(0)) {
                (Some(SourceOperator::Deref), Some(operand)) => {
                    let operand_term = self.term(operand);
                    self.rows.deferred.push(DeferredRow::Deref {
                        node,
                        operand: operand_term,
                        result: own,
                    });
                    self.touch(operand);
                }
                (Some(SourceOperator::Negate | SourceOperator::Not), Some(operand)) if fact.numeric_prefix() => {
                    let operand_term = self.term(operand);
                    self.equate(node, own, operand_term);
                    self.touch(operand);
                }
                _ => self.hole(node, "prefix operator"),
            },
            FactKind::Binary => match (fact.operation(), fact.operand(0), fact.operand(1)) {
                (Some(op @ (SourceOperator::Arithmetic | SourceOperator::Comparison)), Some(lhs), Some(rhs)) => {
                    let lhs_term = self.term(lhs);
                    let rhs_term = self.term(rhs);
                    let row_kind = if op == SourceOperator::Comparison {
                        self.equate(node, own.clone(), RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Bool)));
                        BinaryRowKind::Comparison
                    } else {
                        BinaryRowKind::Arithmetic
                    };
                    self.rows.deferred.push(DeferredRow::Binary {
                        node,
                        kind: row_kind,
                        lhs: lhs_term,
                        rhs: rhs_term,
                        result: own,
                    });
                    self.touch(lhs);
                    self.touch(rhs);
                }
                (Some(SourceOperator::Lazy), Some(lhs), Some(rhs)) => {
                    let bool_term = RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Bool));
                    let lhs_term = self.term(lhs);
                    let rhs_term = self.term(rhs);
                    self.equate(node, lhs_term, bool_term.clone());
                    self.equate(node, rhs_term, bool_term.clone());
                    self.equate(node, own, bool_term);
                    self.touch(lhs);
                    self.touch(rhs);
                }
                _ => self.hole(node, "binary operator"),
            },
            FactKind::Path => ready!(self.path(db, node, own)),
            FactKind::Call => ready!(self.call_site(db, node)),
            // `return` is `!`; its operand is related in `outward`.
            FactKind::Return | FactKind::Discard | FactKind::Wildcard => {}
            other => self.hole(node, format!("expression kind {other:?}")),
        }
        QueryPoll::Ready(Ok(()))
    }

    /// A path expression: a local binding or a parameter of the executing
    /// function; every other value path is a hole.
    fn path(&mut self, db: &mut Database, node: NodeId, own: RowTerm) -> QueryPoll<()> {
        let observed = ready!(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(entry(&self.rows.key, node))));
        let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("component path query variant differs")));
        };
        let local = selected_local(observed);
        if selection_pending(observed) && local.is_none() {
            self.hole(node, "value path selection pending");
            return QueryPoll::Ready(Ok(()));
        }
        let Some(identity) = local else {
            self.hole(node, "non-local value path");
            return QueryPoll::Ready(Ok(()));
        };
        let key = self.rows.key.clone();
        let plan = self.plan;
        match ready!(binding_pattern(db, &key, plan, node, identity)) {
            Some(pattern) => {
                let local = self.local(identity);
                self.equate(node, own, local);
                if !self.enrolled.contains(&identity) {
                    ready!(self.declare_owner(db, pattern));
                    ready!(self.enroll_uses(db, identity, pattern));
                }
            }
            None => {
                // A parameter of the executing function: its declared Type.
                let (term, deferred) = ready!(self.guarded(|c| c.own_parameter_term(db, node)));
                match (term, deferred) {
                    (Some(term), false) if crate::frontend::call_contracts::closed_term(&term) => {
                        self.equate(node, own, RowTerm::Closed(region_free(&term)));
                    }
                    _ => self.hole(node, "parameter type"),
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// Relate a local through its declaring pattern's owner (`let` or match
    /// arm); any other owner (parameter, closure, loop) is a hole.
    fn declare_owner(&mut self, db: &mut Database, pattern: NodeId) -> QueryPoll<()> {
        let plan = self.plan;
        let Some(occurrence) = plan.occurrence(pattern) else {
            self.hole(pattern, "binding without a pattern occurrence");
            return QueryPoll::Ready(Ok(()));
        };
        let root = occurrence.root;
        let owner = plan.uses(root).iter().copied().find(|edge| {
            matches!(
                (plan.fact(edge.parent).map(|fact| fact.kind()), edge.ordinal % 2),
                (Some(FactKind::Let), 0) | (Some(FactKind::Match), 1)
            )
        });
        match owner {
            Some(owner) => ready!(self.pattern_group(db, owner)),
            None => self.hole(pattern, "binding owner"),
        }
        QueryPoll::Ready(Ok(()))
    }
    /// Every use of the local belongs to the component (once per local).
    fn enroll_uses(&mut self, db: &mut Database, identity: Digest, pattern: NodeId) -> QueryPoll<()> {
        let plan = self.plan;
        if !self.enrolled.insert(identity) {
            return QueryPoll::Ready(Ok(()));
        }
        let Some((_, row)) = plan.name_of(pattern) else {
            self.hole(pattern, "binding name row");
            return QueryPoll::Ready(Ok(()));
        };
        let parameter = plan.fact(pattern).is_some_and(|fact| fact.parameter().is_some());
        if row.hidden().iter().any(|region| parameter || *region > pattern) {
            self.hole(pattern, "hidden use");
        }
        let key = self.rows.key.clone();
        for use_node in row.paths().iter().copied() {
            let observed = ready!(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(entry(&key, use_node))));
            let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("component use query variant differs")));
            };
            let selected = selected_local(observed);
            if selection_pending(observed) && selected.is_none() {
                self.hole(use_node, "use selection pending");
                continue;
            }
            if selected == Some(identity) {
                self.touch(use_node);
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// The scrutinee (match) or initializer (let) matched against a root
    /// pattern: relate the pattern's bindings to its Type.
    fn pattern_group(&mut self, db: &mut Database, owner: SourceUse) -> QueryPoll<()> {
        let plan = self.plan;
        let parent = owner.parent;
        let root = owner.child;
        let Some(fact) = plan.fact(parent) else {
            return QueryPoll::Ready(Err(fault("component pattern owner absent")));
        };
        let scrutinee = match fact.kind() {
            FactKind::Let => fact.operand(1),
            FactKind::Match => fact.operand(0),
            _ => None,
        };
        let Some(scrutinee) = scrutinee else {
            self.hole(root, "pattern without a matched value");
            return QueryPoll::Ready(Ok(()));
        };
        if !self.groups_done.insert(root) {
            return QueryPoll::Ready(Ok(()));
        }
        let matched = self.term(scrutinee);
        self.touch(scrutinee);
        if fact.kind() == FactKind::Let
            && let Some(annotation) = fact.annotation_node()
        {
            let (term, deferred) = ready!(self.guarded(|c| c.written_term(db, annotation)));
            if deferred || !crate::frontend::call_contracts::closed_term(&term) {
                self.hole(parent, "let annotation");
            } else {
                self.equate(parent, matched.clone(), RowTerm::Closed(region_free(&term)));
            }
        }
        let Some(tree) = plan.tree(root) else {
            self.hole(root, "pattern tree");
            return QueryPoll::Ready(Ok(()));
        };
        self.match_pattern(db, root, tree.root(), matched)
    }

    fn match_pattern(
        &mut self,
        db: &mut Database,
        root: NodeId,
        node: &PatternNode,
        matched: RowTerm,
    ) -> QueryPoll<()> {
        match node {
            PatternNode::Wildcard => {}
            PatternNode::Binding {
                occurrence,
                binding,
                by_reference,
                subpattern,
                ..
            } => {
                let key = self.rows.key.clone();
                match (ready!(pattern_binding(db, &key, *occurrence)), binding, *by_reference) {
                    (Some(identity), Some(_), false) => {
                        let local = self.local(identity);
                        self.equate(*occurrence, local, matched.clone());
                        ready!(self.enroll_uses(db, identity, *occurrence));
                        if let Some(subpattern) = subpattern {
                            ready!(self.match_pattern(db, root, subpattern, matched));
                        }
                    }
                    (Some(_), _, true) => self.hole(*occurrence, "reference binding"),
                    _ => self.hole(*occurrence, "name pattern"),
                }
            }
            PatternNode::Tuple { items, rest: None } => {
                let mut members = Vec::new();
                for _ in items.iter() {
                    members.push(RowTerm::Var(self.fresh(RowFamily::Type)));
                }
                self.equate(root, matched, RowTerm::Tuple(members.clone()));
                for (item, member) in items.iter().zip(members) {
                    ready!(self.match_pattern(db, root, item, member));
                }
            }
            PatternNode::Reference { mutable, inner, .. } => {
                let target = RowTerm::Var(self.fresh(RowFamily::Type));
                self.equate(
                    root,
                    matched,
                    RowTerm::Ref {
                        mutable: *mutable,
                        inner: Box::new(target.clone()),
                    },
                );
                ready!(self.match_pattern(db, root, inner, target));
            }
            PatternNode::Literal(PatternLiteral::Integer { suffix, .. }) => match suffix {
                Some(primitive) => self.equate(root, matched, RowTerm::Closed(TyTerm::Primitive(*primitive))),
                None => {
                    let literal = RowTerm::Var(self.fresh(RowFamily::Integer));
                    self.equate(root, matched, literal);
                }
            },
            PatternNode::Or(alternatives) => {
                for alternative in alternatives.iter() {
                    ready!(self.match_pattern(db, root, alternative, matched.clone()));
                }
            }
            _ => self.hole(root, "pattern form"),
        }
        QueryPoll::Ready(Ok(()))
    }

    /// A call to a selected source function: one site, every relation of its
    /// declared signature at once (a shared generic parameter links them).
    fn call_site(&mut self, db: &mut Database, call: NodeId) -> QueryPoll<()> {
        if !self.sites_done.insert(call) {
            return QueryPoll::Ready(Ok(()));
        }
        let plan = self.plan;
        let fact = plan.fact(call).expect("checked fact");
        let (callee, deferred) = ready!(self.guarded(|c| c.callee(db, call)));
        let definition = match callee {
            Some(super::Callee::Source(definition)) if !deferred => definition,
            // A diverging closed facade function (`!`) states nothing.
            Some(super::Callee::Facade(function)) if !deferred && function.diverges() => {
                return QueryPoll::Ready(Ok(()));
            }
            _ => {
                self.hole(call, "callee");
                return QueryPoll::Ready(Ok(()));
            }
        };
        // The callee's generic Type parameters: one variable each.
        let signature = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("component callee signature variant differs")));
        };
        let mut substitution = crate::traits::subst::Substitution::default();
        let mut generics = Vec::new();
        for parameter in &signature.generics {
            if parameter.kind != GenericKind::Type {
                continue;
            }
            let key = ParamKey {
                definition: signature.definition,
                ordinal: parameter.ordinal,
            };
            let var = self.fresh(RowFamily::Type);
            generics.push((key, var));
            if substitution
                .insert(key, GenericArg::Type(TyTerm::Infer(self.var_key(var))))
                .is_err()
            {
                self.hole(call, "callee generic substitution");
                return QueryPoll::Ready(Ok(()));
            }
        }
        let arguments: Vec<Option<NodeId>> = fact.operands().iter().skip(1).copied().collect();
        for (ordinal, argument) in arguments.iter().enumerate() {
            let Some(argument) = argument else {
                self.hole(call, "call with a missing argument");
                return QueryPoll::Ready(Ok(()));
            };
            let (term, deferred) =
                ready!(self.guarded(|c| c.parameter_term(db, call, definition, ordinal as u32)));
            let Some(term) = term.filter(|_| !deferred) else {
                self.hole(*argument, "callee parameter type");
                continue;
            };
            match self.instantiate(&substitution, &term) {
                Some(formal) => {
                    let actual = self.term(*argument);
                    // Only an argument sharing a site variable with the rest
                    // of the signature joins the component through this site.
                    let linked = has_var(&formal, self.rows.transaction);
                    self.equate(*argument, actual, formal);
                    if linked {
                        self.touch(*argument);
                    }
                }
                None => self.hole(*argument, "callee parameter term"),
            }
        }
        let (result, deferred) = ready!(self.guarded(|c| c.result_term(db, call, definition)));
        let own = self.term(call);
        match result.filter(|_| !deferred) {
            Some(term) => match self.instantiate(&substitution, &term) {
                Some(formal) => {
                    let linked = has_var(&formal, self.rows.transaction);
                    self.equate(call, own, formal);
                    if linked {
                        self.touch(call);
                    }
                }
                None => self.hole(call, "callee result term"),
            },
            None => self.hole(call, "callee result type"),
        }
        self.rows.sites.push(RowSite {
            node: call,
            callee: definition,
            generics,
        });
        QueryPoll::Ready(Ok(()))
    }

    /// A declared signature term with the callee's Type parameters replaced by
    /// this site's variables (and its regions erased). `None` when the term
    /// still names anything the rows cannot state.
    fn instantiate(&mut self, substitution: &crate::traits::subst::Substitution, term: &TyTerm) -> Option<RowTerm> {
        let term = region_free(&substitution.apply(&region_free(term)).ok()?);
        Some(self.from_term(&term))
    }
    fn from_term(&mut self, term: &TyTerm) -> RowTerm {
        match term {
            TyTerm::Infer(key) if key.transaction == self.rows.transaction => RowTerm::Var(key.ordinal),
            TyTerm::Tuple(members) if !members.is_empty() => {
                RowTerm::Tuple(members.iter().map(|member| self.from_term(member)).collect())
            }
            TyTerm::Ref(_, mutable, inner) => RowTerm::Ref {
                mutable: *mutable,
                inner: Box::new(self.from_term(inner)),
            },
            // Variables nested in a declared nominal or array stay inside the
            // term under their own keys.
            other => RowTerm::Closed(other.clone()),
        }
    }

    /// What each consumer of `node`'s value says about its Type.
    fn outward(
        &mut self,
        db: &mut Database,
        node: NodeId,
        edge: SourceUse,
        consumer: SourceRelationConsumer,
    ) -> QueryPoll<()> {
        let plan = self.plan;
        let parent = edge.parent;
        let Some(parent_fact) = plan.fact(parent) else {
            return QueryPoll::Ready(Err(fault("component consumer absent")));
        };
        match (parent_fact.kind(), consumer) {
            // The parent's own relations include this operand.
            (FactKind::Parenthesis | FactKind::Block | FactKind::Borrow | FactKind::RawBorrow | FactKind::Tuple | FactKind::Prefix | FactKind::Binary, _) => {
                self.touch(parent);
            }
            // A condition is `bool`; it relates nothing else of the `if`.
            (FactKind::If, _) if edge.ordinal == 0 => {
                let term = self.term(node);
                self.equate(parent, term, RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Bool)));
            }
            (FactKind::If, _) => self.touch(parent),
            (FactKind::Match, SourceRelationConsumer::MatchValue) => self.touch(parent),
            // The matched value of a match: its arm patterns relate to it.
            (FactKind::Match, _) if edge.ordinal == 0 => {
                for pattern in parent_fact.operands().iter().skip(1).step_by(2).flatten() {
                    let arm = SourceUse {
                        child: *pattern,
                        parent,
                        ordinal: 1,
                    };
                    ready!(self.pattern_group(db, arm));
                }
            }
            (FactKind::Let, _) if edge.ordinal == 1 => {
                if let Some(pattern) = parent_fact.operand(0) {
                    let owner = SourceUse {
                        child: pattern,
                        parent,
                        ordinal: 0,
                    };
                    ready!(self.pattern_group(db, owner));
                }
            }
            (FactKind::Call, _) if edge.ordinal > 0 => ready!(self.call_site(db, parent)),
            (FactKind::Return, _) => {
                let definition = self.rows.key.body.definition;
                let (result, deferred) = ready!(self.guarded(|c| c.result_term(db, parent, definition)));
                let operand = self.term(node);
                match result.filter(|term| !deferred && crate::frontend::call_contracts::closed_term(term)) {
                    Some(term) => self.equate(parent, operand, RowTerm::Closed(region_free(&term))),
                    None => self.hole(parent, "return type"),
                }
            }
            (FactKind::Discard, _) => {}
            (other, _) => self.hole(parent, format!("consumer kind {other:?}")),
        }
        QueryPoll::Ready(Ok(()))
    }
}

fn term_has_var(term: &TyTerm, transaction: Digest) -> bool {
    match term {
        TyTerm::Infer(key) => key.transaction == transaction,
        TyTerm::Tuple(members) => members.iter().any(|member| term_has_var(member, transaction)),
        TyTerm::Array(inner, _) | TyTerm::Slice(inner) | TyTerm::Ref(_, _, inner) => term_has_var(inner, transaction),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(ty) => term_has_var(ty, transaction),
            _ => false,
        }),
        _ => false,
    }
}
fn has_var(term: &RowTerm, transaction: Digest) -> bool {
    match term {
        RowTerm::Var(_) => true,
        RowTerm::Closed(term) => term_has_var(term, transaction),
        RowTerm::Tuple(members) => members.iter().any(|member| has_var(member, transaction)),
        RowTerm::Ref { inner, .. } => has_var(inner, transaction),
    }
}

/// The transaction scoping one component's variables, by its entry.
pub(crate) fn variable_transaction(key: &PcKey) -> Digest {
    let mut scope = crate::core::digest::StableEncoder::new("lrr/v1/component-variables");
    scope.record(key);
    scope.finish()
}

/// The rows of the numeric literal at `key` (an unsuffixed `Integer`/`Float`
/// node), or `None` when `key` is not such a literal.
pub(crate) fn build(db: &mut Database, key: &PcKey) -> QueryPoll<Option<ComponentRows>> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("component rows require an entry phase")));
    }
    let facts = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(key.body.clone())));
    let QueryValue::SourceConstraints(plan) = facts.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("component rows source plan variant differs")));
    };
    let plan: &SourceConstraintPlan = plan;
    if let Err(error) = plan.relations().require_source(&key.body, plan.structure()) {
        return QueryPoll::Ready(Err(error));
    }
    let leaf = NodeId(key.node);
    let floating = match plan.fact(leaf).map(|fact| fact.kind()) {
        Some(FactKind::Integer) => false,
        Some(FactKind::Float) => true,
        _ => return QueryPoll::Ready(Ok(None)),
    };
    let mut component = Component::new(key, plan, floating);
    component.literal_start = true;
    let mut builder = Builder {
        c: component,
        plan,
        rows: ComponentRows {
            key: key.clone(),
            structure: plan.structure(),
            transaction: variable_transaction(key),
            leaf: 0,
            vars: Vec::new(),
            literals: Vec::new(),
            equations: Vec::new(),
            deferred: Vec::new(),
            sites: Vec::new(),
            holes: Vec::new(),
        },
        node_var: BTreeMap::new(),
        local_var: BTreeMap::new(),
        touched: BTreeSet::new(),
        sites_done: BTreeSet::new(),
        groups_done: BTreeSet::new(),
        enrolled: BTreeSet::new(),
        work: Vec::new(),
    };
    let leaf_term = builder.term(leaf);
    builder.rows.leaf = match leaf_term {
        RowTerm::Var(var) => var,
        _ => return QueryPoll::Ready(Err(fault("component leaf is not a variable"))),
    };
    builder.touch(leaf);
    while let Some(node) = builder.work.pop() {
        ready!(builder.process(db, node));
    }
    QueryPoll::Ready(Ok(Some(builder.rows)))
}
