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
use super::{
    Component, RECURSION_LIMIT, Step, binding_pattern, entry, fault, pattern_binding, selected_local,
    selection_pending,
};
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
    traits::term::{GenericArg, InferVarKey, ParamKey, RegionTerm, TyTerm},
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
    /// A const generic argument (an array length); never a row Type.
    Const,
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
    /// `lhs op= rhs` on builtin numeric Types: the operands share one Type.
    Compound,
    /// Builtin `lhs << rhs` / `lhs >> rhs`: the result is the left operand's
    /// Type; the operands are otherwise independent integers.
    Shift,
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
    /// The callee's const generic parameters and their variables.
    pub(crate) constants: Vec<(ParamKey, u32)>,
}

/// What a structural probe looks up once the base's Type is resolved.
#[derive(Debug, Clone)]
pub(super) enum ProbeKind {
    /// `receiver.name(args)`: the method the dispatcher's probe selects for
    /// the receiver's resolved Type.
    Method { name: String },
    /// `base.field`: the field of the base's autoderef'd Type.
    Field { step: Step },
    /// `base[index]`: the element of the base's autoderef'd array or slice, or
    /// the `Output` of its `Index` impl.
    Index { index: NodeId },
    /// A coercion site (call argument, constructor field, annotated `let`,
    /// `return`): the value of type `base` is coerced to the reference Type
    /// `formal` by rustc's `coerce_borrowed_pointer` (reborrow, autoderef and
    /// array unsizing of the referent).
    Coerce { formal: RowTerm },
}

/// A relation family that needs the pinned owner's resolved Type of `base`
/// before it can state its rows (rustc's method probe and field lookup run at
/// `structurally_resolve_type`). Rows are added after a solve reports the
/// base's Type; the component is re-solved with them.
#[derive(Debug, Clone)]
pub(super) struct Probe {
    pub(super) node: NodeId,
    pub(super) kind: ProbeKind,
    pub(super) base: RowTerm,
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
    /// `core::ops::Deref`, when a dereference row needs the overloaded step.
    pub(crate) deref_trait: Option<Digest>,
    /// `core::cmp::PartialEq`, when a comparison may not be builtin.
    pub(crate) eq_trait: Option<Digest>,
    /// `core::iter::IntoIterator`, when a `for` loop's pattern takes its `Item`.
    pub(crate) into_iter_trait: Option<Digest>,
    /// `core::ops::Index`, when an index expression takes its `Output`.
    pub(crate) index_trait: Option<Digest>,
    /// Probes still waiting for their base's resolved Type.
    pub(super) probes: Vec<Probe>,
}

impl ComponentRows {
    /// Whether the rows are complete: no typed hole and no probe still waiting
    /// for its base's Type, so rustc's fallback phase may run.
    pub(crate) fn is_closed(&self) -> bool {
        self.holes.is_empty() && self.probes.is_empty()
    }
    /// Probes still waiting for their base's Type.
    pub(crate) fn probes_pending(&self) -> usize {
        self.probes.len()
    }
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
    deref_probed: bool,
    eq_probed: bool,
    into_iter_probed: bool,
    probe_nodes: BTreeSet<NodeId>,
    coerced: BTreeSet<NodeId>,
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
        if std::env::var_os("LRR_SCR_TRACE").is_some() {
            eprintln!("SCR-TRACE process node={} kind={kind:?}", node.0);
        }
        ready!(self.inward(db, node, kind));
        // An assignment is `()`, a closed Type: no consumer can relate it to a
        // variable of this component.
        if kind == FactKind::Binary
            && matches!(
                fact.operation(),
                Some(SourceOperator::Assign | SourceOperator::CompoundArithmetic | SourceOperator::CompoundShift)
            )
        {
            return QueryPoll::Ready(Ok(()));
        }
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
                    if !self.deref_probed {
                        self.deref_probed = true;
                        self.rows.deref_trait = ready!(self.c.core_trait(db, "ops", "Deref"));
                    }
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
                        if !self.eq_probed {
                            self.eq_probed = true;
                            self.rows.eq_trait = ready!(self.c.core_trait(db, "cmp", "PartialEq"));
                        }
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
                // `place = value`: both sides share one Type; the expression is `()`.
                (Some(SourceOperator::Assign), Some(lhs), Some(rhs)) => {
                    let lhs_term = self.term(lhs);
                    let rhs_term = self.term(rhs);
                    self.equate(node, lhs_term, rhs_term);
                    self.equate(node, own, RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Unit)));
                    self.touch(lhs);
                    self.touch(rhs);
                }
                // `lhs << rhs`: the result has the left operand's Type.
                (Some(SourceOperator::Shift), Some(lhs), Some(rhs)) => {
                    let lhs_term = self.term(lhs);
                    let rhs_term = self.term(rhs);
                    self.rows.deferred.push(DeferredRow::Binary {
                        node,
                        kind: BinaryRowKind::Shift,
                        lhs: lhs_term,
                        rhs: rhs_term,
                        result: own,
                    });
                    self.touch(lhs);
                    self.touch(rhs);
                }
                // `place <<= value`: the expression is `()`; the operands are
                // independent integers.
                (Some(SourceOperator::CompoundShift), Some(lhs), Some(rhs)) => {
                    self.equate(node, own, RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Unit)));
                    self.touch(lhs);
                    self.touch(rhs);
                }
                // `place op= value` on builtin numeric Types; the expression is `()`.
                (Some(SourceOperator::CompoundArithmetic), Some(lhs), Some(rhs)) => {
                    let lhs_term = self.term(lhs);
                    let rhs_term = self.term(rhs);
                    self.equate(node, own.clone(), RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Unit)));
                    self.rows.deferred.push(DeferredRow::Binary {
                        node,
                        kind: BinaryRowKind::Compound,
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
            FactKind::Record => ready!(self.record_rows(db, node)),
            // `a..=b` is `RangeInclusive::new(a, b)`.
            FactKind::LangCall => {
                if self.sites_done.insert(node) {
                    let (function, _) = ready!(self.guarded(|c| c.lang_function(db, node)));
                    match function {
                        Some(function) => {
                            let arguments: Vec<(u32, NodeId)> = fact
                                .operands()
                                .iter()
                                .enumerate()
                                .filter_map(|(ordinal, operand)| Some((u32::try_from(ordinal).ok()?, (*operand)?)))
                                .collect();
                            if arguments.len() != fact.operands().len() {
                                self.hole(node, "language call with a missing argument");
                            } else {
                                ready!(self.site_rows(db, node, function, None, &arguments));
                            }
                        }
                        None => self.hole(node, "language call callee"),
                    }
                }
            }
            // The loop's value is `()`; its head and pattern relate through
            // `IntoIterator::Item` (see `pattern_group`).
            FactKind::For => {}
            FactKind::Array => {
                let operands: Vec<Option<NodeId>> = fact.operands().to_vec();
                let length = if fact.flag() {
                    // A repeat expression's count is a constant only its
                    // evaluation states.
                    self.hole(node, "repeat array length");
                    if let Some(Some(element)) = operands.first() {
                        self.touch(*element);
                    }
                    return QueryPoll::Ready(Ok(()));
                } else {
                    u64::try_from(operands.len()).ok()
                };
                let element = RowTerm::Var(self.fresh(RowFamily::Type));
                for operand in operands.iter().flatten() {
                    let term = self.term(*operand);
                    self.equate(*operand, term, element.clone());
                    self.touch(*operand);
                }
                match length {
                    Some(length) => {
                        let array = TyTerm::Array(
                            Box::new(self.to_ty(&element)),
                            crate::traits::term::ConstTerm::Value(length),
                        );
                        self.equate(node, own, RowTerm::Closed(array));
                    }
                    None => self.hole(node, "array length"),
                }
            }
            FactKind::Index => match (fact.operand(0), fact.operand(1)) {
                (Some(base), Some(index)) => {
                    self.add_probe(node, ProbeKind::Index { index }, base);
                    self.touch(index);
                }
                _ => self.hole(node, "index expression"),
            },
            // `operand as T` is the written Type; rustc's expectation types an
            // unsuffixed literal operand by the cast target (check_lit).
            FactKind::Cast => {
                let (term, deferred) = ready!(self.guarded(|c| c.annotation_term(db, node)));
                let term = term.and_then(|term| self.fill_holes(&term));
                match term.filter(|term| !deferred && closed_with_vars(term, self.rows.transaction)) {
                    Some(target) => {
                        let target = region_free(&target);
                        let formal = self.from_term(&target);
                        self.equate(node, own, formal);
                        if let Some(operand) = fact.operand(0) {
                            let literal = self.literal_operand(operand);
                            let expectation = match (literal, &target) {
                                (Some(false), TyTerm::Primitive(p)) if super::family(*p) == Some(false) => Some(target.clone()),
                                (Some(false), TyTerm::Primitive(PrimitiveType::Char)) => {
                                    Some(TyTerm::Primitive(PrimitiveType::U8))
                                }
                                (Some(true), TyTerm::Primitive(p)) if super::family(*p) == Some(true) => Some(target.clone()),
                                _ => None,
                            };
                            let operand_term = self.term(operand);
                            if let Some(expectation) = expectation {
                                self.equate(operand, operand_term, RowTerm::Closed(expectation));
                            }
                            self.touch(operand);
                        }
                    }
                    None => self.hole(node, "cast target"),
                }
            }
            FactKind::MethodCall => match (
                fact.operand(0),
                fact.member().and_then(|row| plan.member(row)),
            ) {
                (Some(receiver), Some(name)) => {
                    self.add_probe(node, ProbeKind::Method { name: name.to_owned() }, receiver)
                }
                _ => self.hole(node, "method call without a receiver or name"),
            },
            FactKind::Field => {
                let step = match (fact.field(), fact.member()) {
                    (Some(position), _) => Some(Step::Field(position)),
                    (None, Some(row)) => Some(Step::Named(row)),
                    (None, None) => None,
                };
                match (step, fact.operand(0)) {
                    (Some(step), Some(base)) => self.add_probe(node, ProbeKind::Field { step }, base),
                    _ => self.hole(node, "field access"),
                }
            }
            // `true`, `'c'`, `b'c'`, `"s"` and `b"s"` have their lexical Type.
            FactKind::Literal => {
                let literal = if let Some(length) = fact.byte_length() {
                    Some(TyTerm::Ref(
                        RegionTerm::Static,
                        false,
                        Box::new(TyTerm::Array(
                            Box::new(TyTerm::Primitive(PrimitiveType::U8)),
                            crate::traits::term::ConstTerm::Value(length),
                        )),
                    ))
                } else {
                    match (fact.suffix(), fact.flag()) {
                        (Some(primitive), _) => Some(TyTerm::Primitive(primitive)),
                        (None, true) => Some(TyTerm::Ref(RegionTerm::Static, false, Box::new(TyTerm::Str))),
                        (None, false) => None,
                    }
                };
                match literal {
                    Some(term) => self.equate(node, own, RowTerm::Closed(term)),
                    None => self.hole(node, "literal Type"),
                }
            }
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
            // A fieldless constructor named as a value (`None`, a unit struct).
            if ready!(self.constructor_rows(db, node, &[], None)) {
                return QueryPoll::Ready(Ok(()));
            }
            // A `const` or `static`: its declared Type.
            if let Some(declaration) = observed.declaration() {
                let identity = declaration.identity();
                let (term, deferred) =
                    ready!(self.guarded(|c| c.declared_item_type(db, observed, identity)));
                if let Some(term) = term
                    .filter(|term| !deferred && crate::frontend::call_contracts::closed_term(term))
                {
                    self.equate(node, own, RowTerm::Closed(region_free(&term)));
                    return QueryPoll::Ready(Ok(()));
                }
            }
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
                (Some(FactKind::Let), 0) | (Some(FactKind::Match), 1) | (Some(FactKind::For), 1)
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
            FactKind::For => fact.operand(0),
            _ => None,
        };
        let Some(scrutinee) = scrutinee else {
            self.hole(root, "pattern without a matched value");
            return QueryPoll::Ready(Ok(()));
        };
        if !self.groups_done.insert(root) {
            return QueryPoll::Ready(Ok(()));
        }
        let mut matched = self.term(scrutinee);
        self.touch(scrutinee);
        if fact.kind() == FactKind::For {
            // `for PAT in HEAD`: PAT matches `<HEAD as IntoIterator>::Item`.
            if !self.into_iter_probed {
                self.into_iter_probed = true;
                self.rows.into_iter_trait = ready!(self.c.core_trait(db, "iter", "IntoIterator"));
            }
            let Some(trait_key) = self.rows.into_iter_trait else {
                self.hole(root, "IntoIterator is not declared");
                return QueryPoll::Ready(Ok(()));
            };
            let item = RowTerm::Var(self.fresh(RowFamily::Type));
            let projection = TyTerm::Projection(Box::new(crate::traits::term::ProjectionKey {
                trait_ref: crate::traits::term::TraitRef {
                    trait_key,
                    self_ty: self.to_ty(&matched),
                    args: Vec::new(),
                },
                item: crate::frontend::member_index::associated_name_key("Item"),
                item_args: Vec::new(),
            }));
            self.equate(root, item.clone(), RowTerm::Closed(projection));
            matched = item;
        }
        if fact.kind() == FactKind::Let
            && let Some(annotation) = fact.annotation_node()
        {
            let (term, deferred) = ready!(self.guarded(|c| c.written_term(db, annotation)));
            match self.fill_holes(&term).filter(|term| !deferred && closed_with_vars(term, self.rows.transaction)) {
                Some(term) => {
                    let formal = self.from_term(&region_free(&term));
                    self.coerce(parent, matched.clone(), formal);
                }
                None => self.hole(parent, "let annotation"),
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
    /// declared signature at once (a shared generic parameter links them). A
    /// tuple-struct or variant constructor call is the constructor's rows.
    fn call_site(&mut self, db: &mut Database, call: NodeId) -> QueryPoll<()> {
        if !self.sites_done.insert(call) {
            return QueryPoll::Ready(Ok(()));
        }
        let plan = self.plan;
        let fact = plan.fact(call).expect("checked fact");
        // A callee path writing generic arguments (`f::<u8>(..)`,
        // `Vec::<u8>::new()`) fixes parameters the declared signature leaves to
        // inference: not yet a row family (the component stays open).
        if fact.flag() {
            self.hole(call, "explicit generic arguments");
            return QueryPoll::Ready(Ok(()));
        }
        let (callee, deferred) = ready!(self.guarded(|c| c.callee(db, call)));
        let definition = match callee {
            Some(super::Callee::Source(definition)) if !deferred => definition,
            // A diverging closed facade function (`!`) states nothing.
            Some(super::Callee::Facade(function)) if !deferred && function.diverges() => {
                return QueryPoll::Ready(Ok(()));
            }
            None => {
                let operands: Vec<(u32, NodeId)> = fact
                    .operands()
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter_map(|(ordinal, operand)| Some((u32::try_from(ordinal - 1).ok()?, (*operand)?)))
                    .collect();
                if operands.len() + 1 != fact.operands().len() {
                    self.hole(call, "call with a missing argument");
                    return QueryPoll::Ready(Ok(()));
                }
                if !ready!(self.constructor_rows(db, call, &operands, None)) {
                    self.hole(call, "callee");
                }
                return QueryPoll::Ready(Ok(()));
            }
            _ => {
                self.hole(call, "callee");
                return QueryPoll::Ready(Ok(()));
            }
        };
        let mut arguments = Vec::new();
        for (ordinal, argument) in fact.operands().iter().skip(1).enumerate() {
            let Some(argument) = argument else {
                self.hole(call, "call with a missing argument");
                return QueryPoll::Ready(Ok(()));
            };
            arguments.push((ordinal as u32, *argument));
        }
        self.site_rows(db, call, definition, None, &arguments)
    }

    /// The rows of one selected function site: the callee's generic Type
    /// parameters have one variable each, the receiver (adjusted by the probe)
    /// and the arguments relate to the declared parameters and the call to the
    /// declared result.
    fn site_rows(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
        receiver: Option<&TyTerm>,
        arguments: &[(u32, NodeId)],
    ) -> QueryPoll<()> {
        // The callee's generic Type parameters: one variable each.
        let signature = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("component callee signature variant differs")));
        };
        let mut substitution = crate::traits::subst::Substitution::default();
        let mut generics = Vec::new();
        let mut constants = Vec::new();
        for parameter in &signature.generics {
            let key = ParamKey {
                definition: signature.definition,
                ordinal: parameter.ordinal,
            };
            let family = match parameter.kind {
                GenericKind::Type => RowFamily::Type,
                GenericKind::Const => RowFamily::Const,
                // Regions are erased in the rows.
                _ => continue,
            };
            let var = self.fresh(family);
            let infer = self.var_key(var);
            let argument = match family {
                RowFamily::Const => {
                    constants.push((key, var));
                    GenericArg::Const(crate::traits::term::ConstTerm::Infer(infer))
                }
                _ => {
                    generics.push((key, var));
                    GenericArg::Type(TyTerm::Infer(infer))
                }
            };
            if substitution.insert(key, argument).is_err() {
                self.hole(call, "callee generic substitution");
                return QueryPoll::Ready(Ok(()));
            }
        }
        if let Some(adjusted) = receiver {
            let (term, deferred) = ready!(self.guarded(|c| c.parameter_term(db, call, definition, 0)));
            let Some(term) = term.filter(|_| !deferred) else {
                self.hole(call, "callee receiver type");
                return QueryPoll::Ready(Ok(()));
            };
            let Some(formal) = self.instantiate(&substitution, &term) else {
                self.hole(call, "callee receiver term");
                return QueryPoll::Ready(Ok(()));
            };
            let actual = self.from_term(adjusted);
            self.equate(call, formal, actual);
        }
        for (ordinal, argument) in arguments {
            let (term, deferred) =
                ready!(self.guarded(|c| c.parameter_term(db, call, definition, *ordinal)));
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
                    self.coerce(*argument, actual, formal);
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
            constants,
        });
        QueryPoll::Ready(Ok(()))
    }

    /// The receiver the method probe selected `formal` for: the receiver's
    /// resolved Type, auto-dereferenced (through references and overloaded
    /// `Deref`) and auto-referenced to the first form whose head is the declared
    /// receiver Type's (rustc's probe steps: by value, `&`, `&mut`, then the
    /// next autoderef step).
    fn adjusted_receiver(
        &mut self,
        db: &mut Database,
        receiver: &TyTerm,
        formal: &TyTerm,
        function: Digest,
    ) -> QueryPoll<Option<TyTerm>> {
        let mut current = region_free(receiver);
        for _ in 0..RECURSION_LIMIT {
            for autoref in [None, Some(false), Some(true)] {
                let adjusted = match autoref {
                    None => current.clone(),
                    Some(mutable) => TyTerm::Ref(RegionTerm::Static, mutable, Box::new(current.clone())),
                };
                if shape_match(formal, &adjusted, function) {
                    return QueryPoll::Ready(Ok(Some(adjusted)));
                }
            }
            current = match current {
                TyTerm::Ref(_, _, inner) => *inner,
                TyTerm::Nominal(..) => {
                    let (target, _) = ready!(self.guarded(|c| c.deref_target(db, &current)));
                    match target {
                        Some(target) => region_free(&target),
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                _ => return QueryPoll::Ready(Ok(None)),
            };
        }
        QueryPoll::Ready(Ok(None))
    }

    /// A selected struct or variant constructor (a record expression, a
    /// tuple-constructor call or a fieldless constructor named as a value): the
    /// constructed Type has one variable per declared type parameter, and each
    /// field operand relates to its declared Type under them. `false` when the
    /// syntax does not select a constructor.
    fn constructor_rows(
        &mut self,
        db: &mut Database,
        node: NodeId,
        operands: &[(u32, NodeId)],
        base: Option<NodeId>,
    ) -> QueryPoll<bool> {
        let (constructed, _) = ready!(self.guarded(|c| c.constructor(db, super::nominal::ConstructorSite::Expression(node))));
        let Some(constructed) = constructed else {
            return QueryPoll::Ready(Ok(false));
        };
        // A constructor selected through a written application (`type A = P<u8>`,
        // `Self`) states its arguments there: not yet a row family.
        if constructed.has_application() {
            self.hole(node, "constructor application");
            return QueryPoll::Ready(Ok(true));
        }
        // Explicit written arguments (`P::<u8> { .. }`, `Some::<u8>(..)`) fix
        // their positions; the others are fresh variables.
        let is_record = self.plan.fact(node).is_some_and(|fact| fact.kind() == FactKind::Record);
        let written: Option<Vec<Option<TyTerm>>> = if is_record {
            let bound = ready!(db.fetch(&SyntaxProvider, QueryKey::BoundRecordConstructor(entry(&self.rows.key, node))));
            let QueryValue::BoundRecordConstructor(bound) = bound.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("component record arguments differ")));
            };
            (!bound.arguments().is_empty()).then(|| {
                bound
                    .arguments()
                    .iter()
                    .map(|argument| {
                        argument
                            .as_ref()
                            .filter(|argument| !argument.is_open())
                            .map(|argument| argument.source().clone())
                    })
                    .collect()
            })
        } else {
            let plan = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(self.rows.key.body.clone())));
            let QueryValue::SourceLocalPlan(plan) = plan.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("component constructor plan differs")));
            };
            match ready!(super::super::explicit_type_arguments::select_path_arguments(
                db,
                plan,
                &entry(&self.rows.key, node),
                super::super::explicit_type_arguments::PathArgumentPosition::Constructor,
            )) {
                Some(selected) => {
                    for guard in &selected.guards {
                        if let Err(error) = guard.require_current(db) {
                            return QueryPoll::Ready(Err(error));
                        }
                    }
                    Some(
                        selected
                            .arguments
                            .iter()
                            .map(|argument| argument.as_ref().map(|argument| argument.source().clone()))
                            .collect(),
                    )
                }
                None => None,
            }
        };
        let mut types = Vec::new();
        let mut arguments = Vec::new();
        for position in 0..constructed.type_parameter_count() {
            let explicit = written
                .as_ref()
                .and_then(|written| written.get(position))
                .and_then(|argument| argument.as_ref())
                .map(region_free);
            let term = match explicit {
                Some(term) if !mentions_foreign_param(&term, self.rows.key.body.definition) => term,
                Some(_) => {
                    self.hole(node, "explicit constructor argument");
                    return QueryPoll::Ready(Ok(true));
                }
                None => {
                    let var = self.fresh(RowFamily::Type);
                    TyTerm::Infer(self.var_key(var))
                }
            };
            arguments.push(GenericArg::Type(term.clone()));
            types.push(term);
        }
        let own = self.term(node);
        let nominal = constructed.nominal_type_with(types);
        let constructed_term = self.from_term(&nominal);
        let linked = has_var(&constructed_term, self.rows.transaction);
        self.equate(node, own, constructed_term.clone());
        if let Some(base) = base {
            let base_term = self.term(base);
            self.equate(base, base_term, constructed_term);
            self.touch(base);
        }
        for (field, operand) in operands {
            let actual = self.term(*operand);
            match constructed.field_under(*field, &arguments).and_then(|term| {
                let term = region_free(&term);
                (!mentions_foreign_param(&term, self.rows.key.body.definition)).then(|| self.from_term(&term))
            }) {
                Some(formal) => {
                    let linked = has_var(&formal, self.rows.transaction);
                    self.coerce(*operand, actual, formal);
                    if linked {
                        self.touch(*operand);
                    }
                }
                None => self.hole(*operand, "constructor field type"),
            }
        }
        if linked {
            self.touch(node);
        }
        QueryPoll::Ready(Ok(true))
    }

    /// A record expression: its selected constructor's fields.
    fn record_rows(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        let plan = self.plan;
        let fact = plan.fact(node).expect("checked fact");
        let (constructed, _) = ready!(self.guarded(|c| c.constructor(db, super::nominal::ConstructorSite::Expression(node))));
        let Some(constructed) = constructed else {
            self.hole(node, "record constructor");
            return QueryPoll::Ready(Ok(()));
        };
        let rows = plan.record_fields(node).unwrap_or(&[]);
        let mut operands = Vec::with_capacity(rows.len());
        for (ordinal, row) in rows.iter().enumerate() {
            match (self.c.written_field(&constructed, *row), fact.operand(ordinal)) {
                (Some(field), Some(operand)) => operands.push((field, operand)),
                _ => {
                    self.hole(node, "record field");
                    return QueryPoll::Ready(Ok(()));
                }
            }
        }
        let base = fact.operand(rows.len());
        if fact.operands().len() > rows.len() + usize::from(base.is_some()) {
            self.hole(node, "record operands");
            return QueryPoll::Ready(Ok(()));
        }
        if !ready!(self.constructor_rows(db, node, &operands, base)) {
            self.hole(node, "record constructor");
        }
        QueryPoll::Ready(Ok(()))
    }

    /// A declared signature term with the callee's Type parameters replaced by
    /// this site's variables (and its regions erased). `None` when the term
    /// still names anything the rows cannot state.
    fn instantiate(&mut self, substitution: &crate::traits::subst::Substitution, term: &TyTerm) -> Option<RowTerm> {
        let term = region_free(&substitution.apply(&region_free(term)).ok()?);
        if mentions_foreign_param(&term, self.rows.key.body.definition) {
            return None;
        }
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
    /// A written Type with each `_` hole replaced by a fresh variable of this
    /// component (rustc: a written `_` is a fresh inference variable).
    fn fill_holes(&mut self, term: &TyTerm) -> Option<TyTerm> {
        Some(match term {
            term if *term == super::hole() => {
                let var = self.fresh(RowFamily::Type);
                TyTerm::Infer(self.var_key(var))
            }
            TyTerm::Ref(region, mutable, inner) => {
                TyTerm::Ref(region.clone(), *mutable, Box::new(self.fill_holes(inner)?))
            }
            TyTerm::Slice(inner) => TyTerm::Slice(Box::new(self.fill_holes(inner)?)),
            TyTerm::Array(inner, length) => TyTerm::Array(Box::new(self.fill_holes(inner)?), length.clone()),
            TyTerm::Tuple(members) => TyTerm::Tuple(
                members
                    .iter()
                    .map(|member| self.fill_holes(member))
                    .collect::<Option<Vec<_>>>()?,
            ),
            TyTerm::Nominal(id, args) => TyTerm::Nominal(
                *id,
                args.iter()
                    .map(|arg| match arg {
                        GenericArg::Type(ty) => Some(GenericArg::Type(self.fill_holes(ty)?)),
                        other => Some(other.clone()),
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
            other => other.clone(),
        })
    }
    /// A row term as a Type term over this component's variables.
    fn to_ty(&self, row: &RowTerm) -> TyTerm {
        match row {
            RowTerm::Var(ordinal) => TyTerm::Infer(self.var_key(*ordinal)),
            RowTerm::Closed(term) => term.clone(),
            RowTerm::Tuple(members) => TyTerm::Tuple(members.iter().map(|member| self.to_ty(member)).collect()),
            RowTerm::Ref { mutable, inner } => {
                TyTerm::Ref(RegionTerm::Static, *mutable, Box::new(self.to_ty(inner)))
            }
        }
    }

    /// Whether `operand` (through parentheses) is an unsuffixed numeric literal:
    /// `Some(true)` a float, `Some(false)` an integer.
    fn literal_operand(&self, mut operand: NodeId) -> Option<bool> {
        loop {
            let fact = self.plan.fact(operand)?;
            match fact.kind() {
                FactKind::Parenthesis => operand = fact.operand(0)?,
                FactKind::Prefix
                    if matches!(fact.operation(), Some(SourceOperator::Negate | SourceOperator::Not))
                        && fact.numeric_prefix() =>
                {
                    operand = fact.operand(0)?
                }
                FactKind::Integer => return Some(false),
                FactKind::Float => return Some(true),
                _ => return None,
            }
        }
    }

    /// `actual` flows into `formal` at a coercion site. Only a reference
    /// formal can coerce a different Type (`&[T; N]` to `&[T]`, `&String` to
    /// `&str`); every other formal is equated outright. The reference case waits
    /// for the pinned owner's resolved referent of `actual` (a probe).
    fn coerce(&mut self, node: NodeId, actual: RowTerm, formal: RowTerm) {
        match &formal {
            RowTerm::Ref { .. } | RowTerm::Closed(TyTerm::Ref(..)) => {
                if self.coerced.insert(node) {
                    self.rows.probes.push(Probe {
                        node,
                        kind: ProbeKind::Coerce { formal },
                        base: actual,
                    });
                }
            }
            _ => self.equate(node, actual, formal),
        }
    }

    /// Schedule a structural probe of `base`'s resolved Type (see `Probe`).
    fn add_probe(&mut self, node: NodeId, kind: ProbeKind, base: NodeId) {
        if !self.probe_nodes.insert(node) {
            return;
        }
        let base_term = self.term(base);
        self.touch(base);
        self.rows.probes.push(Probe {
            node,
            kind,
            base: base_term,
        });
    }

    /// The probe's base row term (the value that flows into a coercion site).
    fn base_row(&self, probe: &Probe) -> RowTerm {
        probe.base.clone()
    }

    /// The pinned owner's resolved Type of a row term, from a solve response.
    fn value_of(&self, row: &RowTerm, values: &BTreeMap<InferVarKey, TyTerm>) -> Option<TyTerm> {
        Some(match row {
            RowTerm::Var(ordinal) => values.get(&self.var_key(*ordinal))?.clone(),
            RowTerm::Closed(term) => term.clone(),
            RowTerm::Tuple(members) => TyTerm::Tuple(
                members
                    .iter()
                    .map(|member| self.value_of(member, values))
                    .collect::<Option<Vec<_>>>()?,
            ),
            RowTerm::Ref { mutable, inner } => {
                TyTerm::Ref(RegionTerm::Static, *mutable, Box::new(self.value_of(inner, values)?))
            }
        })
    }

    /// One probe round: every pending probe whose base the owner has resolved
    /// states its rows. `true` when any probe was answered (rows or a hole).
    fn resolve_probes(
        &mut self,
        db: &mut Database,
        values: &BTreeMap<InferVarKey, TyTerm>,
    ) -> QueryPoll<bool> {
        let pending = std::mem::take(&mut self.rows.probes);
        let mut waiting = Vec::new();
        let mut progress = false;
        for probe in pending {
            let value = self
                .value_of(&probe.base, values)
                .filter(|value| !matches!(value, TyTerm::Infer(_)));
            let Some(value) = value else {
                waiting.push(probe);
                continue;
            };
            ready!(self.resolve_probe(db, &probe, value));
            progress = true;
        }
        self.rows.probes = waiting;
        QueryPoll::Ready(Ok(progress))
    }

    fn resolve_probe(&mut self, db: &mut Database, probe: &Probe, base: TyTerm) -> QueryPoll<()> {
        let node = probe.node;
        match &probe.kind {
            ProbeKind::Method { name } => {
                let plan = self.plan;
                let fact = plan.fact(node).expect("checked fact");
                let mut arguments = Vec::new();
                for (ordinal, argument) in fact.operands().iter().enumerate().skip(1) {
                    let Some(argument) = argument else {
                        self.hole(node, "method call with a missing argument");
                        return QueryPoll::Ready(Ok(()));
                    };
                    arguments.push((ordinal as u32, *argument));
                }
                // rustc's probe: at each autoderef step of the receiver, the
                // method of that step whose declared receiver Type the step
                // (by value, `&` or `&mut`) fits.
                let mut step = region_free(&base);
                for _ in 0..RECURSION_LIMIT {
                    let (function, _) = ready!(self.guarded(|c| c.method_for_receiver(db, None, &step, name)));
                    if let Some(function) = function {
                        let signature = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(function)));
                        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
                            return QueryPoll::Ready(Err(fault("component method signature variant differs")));
                        };
                        if signature.parameters.first().is_some_and(|parameter| parameter.receiver) {
                            let (declared, deferred) =
                                ready!(self.guarded(|c| c.parameter_term(db, node, function, 0)));
                            if let Some(declared) = declared.filter(|_| !deferred)
                                && let Some(adjusted) =
                                    ready!(self.adjusted_receiver(db, &step, &declared, function))
                            {
                                return self.site_rows(db, node, function, Some(&adjusted), &arguments);
                            }
                        }
                    }
                    step = match step {
                        TyTerm::Ref(_, _, inner) => *inner,
                        TyTerm::Nominal(..) => {
                            let (target, _) = ready!(self.guarded(|c| c.deref_target(db, &step)));
                            match target {
                                Some(target) => region_free(&target),
                                None => break,
                            }
                        }
                        _ => break,
                    };
                }
                self.hole(node, format!("method selection {name}"));
                QueryPoll::Ready(Ok(()))
            }
            ProbeKind::Field { step } => {
                let mut current = region_free(&base);
                for _ in 0..RECURSION_LIMIT {
                    while let TyTerm::Ref(_, _, inner) = current {
                        current = *inner;
                    }
                    if matches!(current, TyTerm::Tuple(_) | TyTerm::Nominal(..)) {
                        let (field, _) = ready!(self.guarded(|c| c.project_term(db, &current, &[*step])));
                        if let Some(field) = field {
                            let field = region_free(&field);
                            if mentions_foreign_param(&field, self.rows.key.body.definition) {
                                self.hole(node, "field type");
                            } else {
                                let own = self.term(node);
                                let formal = self.from_term(&field);
                                self.equate(node, own, formal);
                            }
                            return QueryPoll::Ready(Ok(()));
                        }
                    }
                    if !matches!(current, TyTerm::Nominal(..)) {
                        break;
                    }
                    let (target, _) = ready!(self.guarded(|c| c.deref_target(db, &current)));
                    match target {
                        Some(target) => current = region_free(&target),
                        None => break,
                    }
                }
                self.hole(node, "field");
                QueryPoll::Ready(Ok(()))
            }
            ProbeKind::Coerce { formal } => {
                let formal_term = self.to_ty(formal);
                let (TyTerm::Ref(_, _, formal_inner), TyTerm::Ref(_, _, actual_inner)) = (&formal_term, &base) else {
                    // Not both references: plain unification.
                    let actual = self.base_row(probe);
                    self.equate(node, actual, formal.clone());
                    return QueryPoll::Ready(Ok(()));
                };
                let formal_row = match formal {
                    RowTerm::Ref { inner, .. } => (**inner).clone(),
                    _ => self.from_term(formal_inner),
                };
                let mut current = region_free(actual_inner);
                for _ in 0..RECURSION_LIMIT {
                    // The first step whose head is the formal referent's.
                    if shape_match(formal_inner, &current, Digest::default()) {
                        let actual = self.from_term(&current);
                        self.equate(node, formal_row, actual);
                        return QueryPoll::Ready(Ok(()));
                    }
                    // Unsizing `[T; N]` to `[T]`.
                    if let (TyTerm::Array(element, _), TyTerm::Slice(target)) = (&current, &**formal_inner) {
                        let element = self.from_term(&region_free(element));
                        let target = self.from_term(target);
                        self.equate(node, target, element);
                        return QueryPoll::Ready(Ok(()));
                    }
                    current = match current {
                        TyTerm::Ref(_, _, inner) => *inner,
                        TyTerm::Nominal(..) => {
                            let (target, _) = ready!(self.guarded(|c| c.deref_target(db, &current)));
                            match target {
                                Some(target) => region_free(&target),
                                None => break,
                            }
                        }
                        _ => break,
                    };
                }
                // No coercion found: the types are related as written.
                let actual = self.base_row(probe);
                self.equate(node, actual, formal.clone());
                QueryPoll::Ready(Ok(()))
            }
            ProbeKind::Index { index } => {
                let index = *index;
                let Some(environment) = ready!(self.c.environment(db)) else {
                    self.hole(node, "index environment");
                    return QueryPoll::Ready(Ok(()));
                };
                let members = match environment.member_index() {
                    Ok(members) => members,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                let (index_trait, _) = ready!(self.guarded(|c| c.core_trait(db, "ops", "Index")));
                self.rows.index_trait = index_trait;
                let mut current = region_free(&base);
                for _ in 0..RECURSION_LIMIT {
                    while let TyTerm::Ref(_, _, inner) = current {
                        current = *inner;
                    }
                    match &current {
                        // Builtin indexing of an array or slice by `usize`.
                        TyTerm::Array(element, _) | TyTerm::Slice(element) => {
                            let index_term = self.term(index);
                            self.equate(index, index_term, RowTerm::Closed(TyTerm::Primitive(PrimitiveType::Usize)));
                            let own = self.term(node);
                            let formal = self.from_term(element);
                            self.equate(node, own, formal);
                            return QueryPoll::Ready(Ok(()));
                        }
                        TyTerm::Nominal(identity, _) => {
                            if let Some(trait_key) = index_trait
                                && members.implements(Ok(*identity), trait_key)
                            {
                                let index_ty = {
                                    let term = self.term(index);
                                    self.to_ty(&term)
                                };
                                let output = TyTerm::Projection(Box::new(crate::traits::term::ProjectionKey {
                                    trait_ref: crate::traits::term::TraitRef {
                                        trait_key,
                                        self_ty: current.clone(),
                                        args: vec![GenericArg::Type(index_ty)],
                                    },
                                    item: crate::frontend::member_index::associated_name_key("Output"),
                                    item_args: Vec::new(),
                                }));
                                let own = self.term(node);
                                self.equate(node, own, RowTerm::Closed(output));
                                return QueryPoll::Ready(Ok(()));
                            }
                            let (target, _) = ready!(self.guarded(|c| c.deref_target(db, &current)));
                            match target {
                                Some(target) => current = region_free(&target),
                                None => break,
                            }
                        }
                        _ => break,
                    }
                }
                self.hole(node, "index base");
                QueryPoll::Ready(Ok(()))
            }
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
            (
                FactKind::Parenthesis
                | FactKind::Block
                | FactKind::Borrow
                | FactKind::RawBorrow
                | FactKind::Tuple
                | FactKind::Prefix
                | FactKind::Binary
                | FactKind::Field
                | FactKind::MethodCall
                | FactKind::Record
                | FactKind::Array
                | FactKind::Index
                | FactKind::Cast
                | FactKind::LangCall,
                _,
            ) => {
                self.touch(parent);
            }
            // A `for` head: the loop pattern's bindings take its `Item`.
            (FactKind::For, _) if edge.ordinal == 0 => {
                if let Some(pattern) = parent_fact.operand(1) {
                    let owner = SourceUse {
                        child: pattern,
                        parent,
                        ordinal: 1,
                    };
                    ready!(self.pattern_group(db, owner));
                }
            }
            // The loop body is a block of `()`.
            (FactKind::For, _) => {}
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
                    Some(term) => self.coerce(parent, operand, RowTerm::Closed(region_free(&term))),
                    None => self.hole(parent, "return type"),
                }
            }
            (FactKind::Discard, _) => {}
            // A formatted argument is only read through `Display`/`Debug`: the
            // bounds no impl of which constrains an inference variable beyond
            // its own fallback (rustc registers them as ambiguous obligations).
            (FactKind::FormatArguments, _) => {}
            (other, _) => self.hole(parent, format!("consumer kind {other:?}")),
        }
        QueryPoll::Ready(Ok(()))
    }
}

/// Whether `term` names a generic parameter that is not the executing body's own
/// (rigid) one: a callee parameter its site did not substitute (a trait's
/// `Self`, a const or lifetime parameter), which the rows cannot state.
fn mentions_foreign_param(term: &TyTerm, own: Digest) -> bool {
    match term {
        TyTerm::Param(key) => key.definition != own,
        TyTerm::Opaque(..) => true,
        TyTerm::Projection(projection) => {
            mentions_foreign_param(&projection.trait_ref.self_ty, own)
                || projection.trait_ref.args.iter().any(|arg| arg_foreign(arg, own))
                || projection.item_args.iter().any(|arg| arg_foreign(arg, own))
        }
        TyTerm::Tuple(members) => members.iter().any(|member| mentions_foreign_param(member, own)),
        TyTerm::Array(inner, length) => const_foreign(length, own) || mentions_foreign_param(inner, own),
        TyTerm::Slice(inner) | TyTerm::Ref(_, _, inner) => mentions_foreign_param(inner, own),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| arg_foreign(arg, own)),
        _ => false,
    }
}
fn arg_foreign(arg: &GenericArg, own: Digest) -> bool {
    match arg {
        GenericArg::Type(term) => mentions_foreign_param(term, own),
        GenericArg::Const(value) => const_foreign(value, own),
        _ => false,
    }
}
fn const_foreign(value: &crate::traits::term::ConstTerm, own: Digest) -> bool {
    match value {
        crate::traits::term::ConstTerm::Value(_) => false,
        crate::traits::term::ConstTerm::Param(key) => key.definition != own,
        crate::traits::term::ConstTerm::Infer(_) => false,
        _ => true,
    }
}

/// Whether a declared receiver Type and an adjusted receiver Type have the
/// same outer structure (a variable of either side, and a generic parameter of
/// the selected function `function`, matches any Type).
fn shape_match(formal: &TyTerm, adjusted: &TyTerm, function: Digest) -> bool {
    match (formal, adjusted) {
        (TyTerm::Infer(_), _) | (_, TyTerm::Infer(_)) => true,
        (TyTerm::Param(key), _) if key.definition == function => true,
        (TyTerm::Ref(_, left, left_inner), TyTerm::Ref(_, right, right_inner)) => {
            left == right && shape_match(left_inner, right_inner, function)
        }
        (TyTerm::Nominal(left, _), TyTerm::Nominal(right, _)) => left == right,
        (TyTerm::Primitive(left), TyTerm::Primitive(right)) => left == right,
        (TyTerm::Str, TyTerm::Str) | (TyTerm::Slice(_), TyTerm::Slice(_)) | (TyTerm::Array(..), TyTerm::Array(..)) => {
            true
        }
        (TyTerm::Tuple(left), TyTerm::Tuple(right)) => left.len() == right.len(),
        (TyTerm::Param(left), TyTerm::Param(right)) => left == right,
        (TyTerm::Dyn(left), TyTerm::Dyn(right)) => left == right,
        _ => false,
    }
}

/// Whether `term` is closed except for this component's own variables.
fn closed_with_vars(term: &TyTerm, transaction: Digest) -> bool {
    use crate::traits::term::ConstTerm;
    let constant = |value: &ConstTerm| match value {
        ConstTerm::Value(_) => true,
        ConstTerm::Infer(key) => key.transaction == transaction,
        _ => false,
    };
    match term {
        TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Never | TyTerm::Dyn(_) => true,
        TyTerm::Infer(key) => key.transaction == transaction,
        TyTerm::Ref(region, _, target) => {
            !matches!(region, RegionTerm::Placeholder(_)) && closed_with_vars(target, transaction)
        }
        TyTerm::Nominal(_, args) => args.iter().all(|arg| match arg {
            GenericArg::Type(ty) => closed_with_vars(ty, transaction),
            GenericArg::Region(region) => !matches!(region, RegionTerm::Placeholder(_)),
            GenericArg::Const(value) => constant(value),
        }),
        TyTerm::Tuple(fields) => fields.iter().all(|field| closed_with_vars(field, transaction)),
        TyTerm::Array(element, length) => constant(length) && closed_with_vars(element, transaction),
        TyTerm::Slice(element) => closed_with_vars(element, transaction),
        _ => false,
    }
}

fn term_has_var(term: &TyTerm, transaction: Digest) -> bool {
    match term {
        TyTerm::Infer(key) => key.transaction == transaction,
        TyTerm::Tuple(members) => members.iter().any(|member| term_has_var(member, transaction)),
        TyTerm::Array(inner, length) => const_has_var(length, transaction) || term_has_var(inner, transaction),
        TyTerm::Slice(inner) | TyTerm::Ref(_, _, inner) => term_has_var(inner, transaction),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(ty) => term_has_var(ty, transaction),
            GenericArg::Const(value) => const_has_var(value, transaction),
            _ => false,
        }),
        _ => false,
    }
}
fn const_has_var(value: &crate::traits::term::ConstTerm, transaction: Digest) -> bool {
    matches!(value, crate::traits::term::ConstTerm::Infer(key) if key.transaction == transaction)
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
    build_with(db, key, None)
}

/// One solve of the rows built so far: the pinned owner's resolved Type of each
/// source variable, or `None` when it reached no solution to read.
pub(crate) type SolveRound<'a> =
    &'a mut dyn FnMut(&mut Database, &ComponentRows) -> QueryPoll<Option<BTreeMap<InferVarKey, TyTerm>>>;

/// Build the rows and, given a solver, run the probe rounds: a structural
/// probe (method call, field access) waits for its base's Type, a solve reports
/// it, the probe adds its rows, and the component is solved again until no
/// probe can be answered. The last solve is the component's answer; a probe
/// that never resolves is a hole, so the component is not closed.
pub(crate) fn build_with(
    db: &mut Database,
    key: &PcKey,
    mut solve: Option<SolveRound<'_>>,
) -> QueryPoll<Option<ComponentRows>> {
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
            deref_trait: None,
            eq_trait: None,
            into_iter_trait: None,
            index_trait: None,
            probes: Vec::new(),
        },
        node_var: BTreeMap::new(),
        local_var: BTreeMap::new(),
        touched: BTreeSet::new(),
        sites_done: BTreeSet::new(),
        groups_done: BTreeSet::new(),
        enrolled: BTreeSet::new(),
        deref_probed: false,
        eq_probed: false,
        into_iter_probed: false,
        probe_nodes: BTreeSet::new(),
        coerced: BTreeSet::new(),
        work: Vec::new(),
    };
    let leaf_term = builder.term(leaf);
    builder.rows.leaf = match leaf_term {
        RowTerm::Var(var) => var,
        _ => return QueryPoll::Ready(Err(fault("component leaf is not a variable"))),
    };
    builder.touch(leaf);
    loop {
        while let Some(node) = builder.work.pop() {
            ready!(builder.process(db, node));
        }
        let Some(solve) = solve.as_mut() else { break };
        if std::env::var_os("LRR_SCR_DEBUG").is_some() {
            eprintln!(
                "SCR-DEBUG round node={} vars={} eq={} deferred={} sites={} probes={} holes={}",
                key.node,
                builder.rows.vars.len(),
                builder.rows.equations.len(),
                builder.rows.deferred.len(),
                builder.rows.sites.len(),
                builder.rows.probes.len(),
                builder.rows.holes.len()
            );
        }
        let values = ready!(solve(db, &builder.rows));
        let Some(values) = values else { break };
        if builder.rows.probes.is_empty() {
            break;
        }
        if ready!(builder.resolve_probes(db, &values)) {
            continue;
        }
        // No more progress: a coercion whose referent never resolved unifies as
        // written (rustc: an unresolved source Type unifies with the target).
        let mut converted = false;
        for probe in std::mem::take(&mut builder.rows.probes) {
            match probe.kind {
                ProbeKind::Coerce { formal } => {
                    builder.equate(probe.node, probe.base, formal);
                    converted = true;
                }
                kind => builder.rows.probes.push(Probe { kind, ..probe }),
            }
        }
        if !converted {
            break;
        }
    }
    // A probe that stayed unresolved leaves the component open.
    for probe in std::mem::take(&mut builder.rows.probes) {
        builder.hole(probe.node, "unresolved base type");
    }
    QueryPoll::Ready(Ok(Some(builder.rows)))
}
