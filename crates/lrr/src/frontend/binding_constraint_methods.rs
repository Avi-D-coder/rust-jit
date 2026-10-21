//! Static receiver Types for the source numeric constraint service: method
//! calls select their method with the dispatcher's own probe rule
//! (`MemberIndex::select_method`) from the receiver's Type as the ordinary
//! services state it (written annotations, declared parameters and results,
//! selected constructors and fields), and index operands take the builtin
//! array/slice index Type. An expression whose Type those services do not
//! state yields no selection, which stays a missing goal.
use super::{Component, ConstraintDemand, Step, entry, fault};
use crate::{
    core::digest::Digest,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    facade::functions::LibraryFunction,
    frontend::member_index::BuiltinSelf,
    ir::{op::PrimitiveType, pc::NodeId},
    source::manifest::ItemKind,
    syntax::source_constraints::{FactKind, SourceOperator},
    traits::term::{ConstTerm, InferVarKey, PlaceholderKey, RegionTerm, TyTerm},
};
use std::collections::BTreeMap;

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// A part of a static Type that only inference decides.
pub(super) fn open() -> TyTerm {
    TyTerm::Infer(InferVarKey {
        transaction: Digest::default(),
        ordinal: 0,
    })
}
/// A shared borrow's Type; regions and mutability do not affect selection.
fn borrowed(inner: TyTerm) -> TyTerm {
    TyTerm::Ref(RegionTerm::Static, false, Box::new(inner))
}
/// Whether a declared Type term mentions a generic parameter.
pub(super) fn mentions_parameter(term: &TyTerm) -> bool {
    use crate::traits::term::GenericArg;
    match term {
        TyTerm::Param(_) | TyTerm::Projection(_) | TyTerm::Opaque(..) => true,
        TyTerm::Tuple(members) => members.iter().any(mentions_parameter),
        TyTerm::Array(inner, _) | TyTerm::Slice(inner) | TyTerm::Ref(_, _, inner) => {
            mentions_parameter(inner)
        }
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(term) => mentions_parameter(term),
            _ => false,
        }),
        TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Never => false,
        _ => true,
    }
}
/// Bind each parameter of `definition` that the declared Type `declared`
/// names to the part of the reached Type `actual` at the same position
/// (through references: the receiver and arguments auto-ref and auto-deref).
pub(super) fn bind_parameters(
    declared: &TyTerm,
    actual: &TyTerm,
    definition: Digest,
    out: &mut BTreeMap<crate::traits::term::ParamKey, TyTerm>,
) {
    use crate::traits::term::GenericArg;
    match (declared, actual) {
        (TyTerm::Param(key), actual) if key.definition == definition => {
            out.entry(*key).or_insert_with(|| actual.clone());
        }
        (TyTerm::Ref(_, _, declared), TyTerm::Ref(_, _, actual)) => {
            bind_parameters(declared, actual, definition, out)
        }
        // An auto-ref'd argument or receiver.
        (TyTerm::Ref(_, _, declared), actual) => bind_parameters(declared, actual, definition, out),
        (declared, TyTerm::Ref(_, _, actual)) => bind_parameters(declared, actual, definition, out),
        (TyTerm::Nominal(left, declared), TyTerm::Nominal(right, actual)) if left == right => {
            for (declared, actual) in declared.iter().zip(actual) {
                if let (GenericArg::Type(declared), GenericArg::Type(actual)) = (declared, actual) {
                    bind_parameters(declared, actual, definition, out);
                }
            }
        }
        (TyTerm::Tuple(declared), TyTerm::Tuple(actual)) if declared.len() == actual.len() => {
            for (declared, actual) in declared.iter().zip(actual) {
                bind_parameters(declared, actual, definition, out);
            }
        }
        (
            TyTerm::Array(declared, _) | TyTerm::Slice(declared),
            TyTerm::Array(actual, _) | TyTerm::Slice(actual),
        ) => bind_parameters(declared, actual, definition, out),
        _ => {}
    }
}
/// The autoderef'd receiver self type the probe selects from.
fn probe_self(mut term: &TyTerm) -> Option<Result<Digest, BuiltinSelf>> {
    while let TyTerm::Ref(_, _, inner) = term {
        term = inner;
    }
    Some(match term {
        TyTerm::Nominal(identity, _) => Ok(*identity),
        TyTerm::Primitive(PrimitiveType::Unit) => Err(BuiltinSelf::Tuple(0)),
        TyTerm::Primitive(primitive) => Err(BuiltinSelf::Primitive(*primitive)),
        TyTerm::Str => Err(BuiltinSelf::Str),
        TyTerm::Slice(_) => Err(BuiltinSelf::Slice),
        TyTerm::Array(..) => Err(BuiltinSelf::Array),
        TyTerm::Tuple(items) => Err(BuiltinSelf::Tuple(u32::try_from(items.len()).ok()?)),
        _ => return None,
    })
}
/// rustc's probe at one autoderef step: the methods whose self type is the
/// step's type, else the methods of the autoref'd `&` / `&mut` forms (an
/// `impl Trait for &[T]` method taking `self` by value).
fn probe_methods(
    members: &crate::frontend::member_index::MemberIndex,
    self_type: Result<Digest, BuiltinSelf>,
    name: &str,
) -> Result<crate::frontend::member_index::MethodAlternatives, crate::core::diagnostic::Diagnostic>
{
    let found = members.method_alternatives(self_type, name);
    match &found {
        Err(error) if super::not_closed(error) => {}
        _ => return found,
    }
    for mutable in [false, true] {
        match members.method_alternatives_of(
            Err(BuiltinSelf::Reference(mutable)),
            Some(self_type),
            name,
        ) {
            Ok(alternatives) => return Ok(alternatives),
            Err(error) if super::not_closed(&error) => {}
            Err(error) => return Err(error),
        }
    }
    found
}

impl<'a> Component<'a> {
    /// The static Type of an expression, without recording any relation or
    /// pending goal of this component.
    pub(super) fn expression_type(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        if let Some(known) = self.types.get(&node) {
            return QueryPoll::Ready(Ok(known.clone()));
        }
        // A cycle through a binding's own initializer answers nothing.
        self.types.insert(node, None);
        let saved = (self.pending, self.unrecorded, self.via.clone());
        let result = self.expression_type_inner(db, node);
        (self.pending, self.unrecorded, self.via) = saved;
        let found = match ready!(result) {
            // A declared Type that names a projection of concrete Types
            // (`const C: <S as Tr>::Out`, an own parameter) is the Type the
            // selected impl states.
            Some(term) => Some(ready!(self.normalize_projections(db, &term))),
            None => None,
        };
        self.types.insert(node, found.clone());
        QueryPoll::Ready(Ok(found))
    }
    fn expression_type_inner(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        QueryPoll::Ready(Ok(match fact.kind() {
            FactKind::Parenthesis | FactKind::Block => match fact.operand(0) {
                Some(inner) => ready!(self.expression_type(db, inner)),
                None => None,
            },
            FactKind::Borrow => match fact.operand(0) {
                Some(inner) => ready!(self.expression_type(db, inner)).map(borrowed),
                None => None,
            },
            FactKind::RawBorrow => match fact.operand(0) {
                Some(inner) => ready!(self.expression_type(db, inner))
                    .map(|inner| crate::traits::pointer_terms::raw_pointer(fact.flag(), inner)),
                None => None,
            },
            FactKind::Prefix => {
                let inner = match fact.operand(0) {
                    Some(inner) => ready!(self.expression_type(db, inner)),
                    None => None,
                };
                match (fact.operation(), inner) {
                    (Some(SourceOperator::Deref), Some(TyTerm::Ref(_, _, inner))) => Some(*inner),
                    // `*p` of a raw pointer is its pointee (E-R5-008).
                    (Some(SourceOperator::Deref), Some(term))
                        if crate::traits::pointer_terms::as_raw_pointer(&term).is_some() =>
                    {
                        crate::traits::pointer_terms::as_raw_pointer(&term)
                            .map(|(_, pointee)| pointee.clone())
                    }
                    (
                        Some(SourceOperator::Negate | SourceOperator::Not),
                        Some(term @ (TyTerm::Primitive(_) | TyTerm::Infer(_))),
                    ) => Some(term),
                    _ => None,
                }
            }
            FactKind::Binary => match fact.operation() {
                Some(SourceOperator::Comparison | SourceOperator::Lazy) => {
                    Some(TyTerm::Primitive(PrimitiveType::Bool))
                }
                Some(SourceOperator::Assign | SourceOperator::CompoundArithmetic)
                | Some(SourceOperator::CompoundShift) => {
                    Some(TyTerm::Primitive(PrimitiveType::Unit))
                }
                Some(operation @ (SourceOperator::Arithmetic | SourceOperator::Shift)) => {
                    ready!(self.builtin_binary_type(db, node, operation))
                }
                _ => None,
            },
            FactKind::Integer | FactKind::Float => Some(open()),
            FactKind::SuffixedInteger | FactKind::SuffixedFloat => {
                fact.suffix().map(TyTerm::Primitive)
            }
            // `bool`, `char` and byte literals have their lexical Type; a
            // string literal is `&'static str`.
            FactKind::Literal if fact.byte_length().is_some() => Some(TyTerm::Ref(
                RegionTerm::Static,
                false,
                Box::new(TyTerm::Array(
                    Box::new(TyTerm::Primitive(PrimitiveType::U8)),
                    crate::traits::term::ConstTerm::Value(fact.byte_length().expect("matched byte length")),
                )),
            )),
            FactKind::Literal => match (fact.suffix(), fact.flag()) {
                (Some(primitive), _) => Some(TyTerm::Primitive(primitive)),
                (None, true) => Some(TyTerm::Ref(
                    RegionTerm::Static,
                    false,
                    Box::new(TyTerm::Str),
                )),
                (None, false) => None,
            },
            FactKind::Path => ready!(self.path_type(db, node)),
            FactKind::Record => ready!(self.constructed_type(db, node)),
            FactKind::Call => match ready!(self.constructed_type(db, node)) {
                Some(term) => Some(term),
                None => match ready!(self.callee(db, node)) {
                    Some(super::Callee::Source(definition)) => {
                        match ready!(self.result_term(db, node, definition)) {
                            Some(result) => {
                                let result =
                                    ready!(self.instantiate_result(db, node, definition, result));
                                Some(ready!(self.decided(db, node, result)))
                            }
                            None => None,
                        }
                    }
                    Some(super::Callee::Facade(LibraryFunction::MustUse)) => {
                        match fact.operand(1) {
                            Some(argument) => ready!(self.expression_type(db, argument)),
                            None => None,
                        }
                    }
                    // A callable value's written result Type.
                    Some(super::Callee::Bound(bound)) => bound.result.clone(),
                    _ => None,
                },
            },
            FactKind::MethodCall => match ready!(self.method_function(db, node)) {
                Some(function) => match ready!(self.result_term(db, node, function)) {
                    Some(result) => {
                        let result = ready!(self.instantiate_result(db, node, function, result));
                        Some(ready!(self.decided(db, node, result)))
                    }
                    None => None,
                },
                None => None,
            },
            FactKind::LangCall => ready!(self.lang_type(db, node)),
            FactKind::Field => {
                let step = match (fact.field(), fact.member()) {
                    (Some(position), _) => Step::Field(position),
                    (None, Some(row)) => Step::Named(row),
                    (None, None) => return QueryPoll::Ready(Ok(None)),
                };
                match fact.operand(0) {
                    Some(base) => match ready!(self.expression_type(db, base)) {
                        Some(base) => {
                            ready!(self.project_term(db, &base, &[Step::AutoDeref, step]))
                        }
                        None => None,
                    },
                    None => None,
                }
            }
            // A user type's own `Index<Idx>` impl states the result.
            FactKind::Index => match ready!(self.user_index_output(db, node)) {
                Some(output) => Some(output),
                None if ready!(self.index_is_element(db, node)) => match fact.operand(0) {
                    Some(base) => match ready!(self.expression_type(db, base)) {
                        Some(base) => {
                            ready!(self.project_term(db, &base, &[Step::AutoDeref, Step::Element]))
                        }
                        None => None,
                    },
                    None => None,
                },
                None => None,
            },
            // `base[range]`: the slice of the base's element Type.
            FactKind::IndexRange => match fact.operand(0) {
                Some(base) => match ready!(self.expression_type(db, base)) {
                    Some(base) => {
                        ready!(self.project_term(db, &base, &[Step::AutoDeref, Step::Element]))
                            .map(|element| TyTerm::Slice(Box::new(element)))
                    }
                    None => None,
                },
                None => None,
            },
            FactKind::Array => {
                let element = match fact.operand(0) {
                    Some(first) => ready!(self.expression_type(db, first)).unwrap_or_else(open),
                    None => open(),
                };
                // A repeat expression's count is a constant only its
                // evaluation states; an element list is its own length.
                let length = match u64::try_from(fact.operands().len()) {
                    Ok(length) if !fact.flag() => ConstTerm::Value(length),
                    _ => ConstTerm::Placeholder(PlaceholderKey {
                        transaction: Digest::default(),
                        universe: 0,
                        index: 0,
                    }),
                };
                Some(TyTerm::Array(Box::new(element), length))
            }
            FactKind::Tuple => {
                let mut members = Vec::with_capacity(fact.operands().len());
                for member in fact.operands().to_vec() {
                    members.push(match member {
                        Some(member) => {
                            ready!(self.expression_type(db, member)).unwrap_or_else(open)
                        }
                        None => open(),
                    });
                }
                Some(if members.is_empty() {
                    TyTerm::Primitive(PrimitiveType::Unit)
                } else {
                    TyTerm::Tuple(members)
                })
            }
            FactKind::Cast => ready!(self.annotation_term(db, node)),
            // The value of a `match` or `if` has the Type of its first branch
            // that states one (a diverging branch states none).
            FactKind::Match | FactKind::If => {
                let branches: Vec<Option<NodeId>> = if fact.kind() == FactKind::Match {
                    fact.operands().iter().skip(2).step_by(2).copied().collect()
                } else {
                    vec![fact.operand(1), fact.operand(2)]
                };
                let mut found = None;
                for branch in branches.into_iter().flatten() {
                    if let Some(term) = ready!(self.expression_type(db, branch))
                        && term != TyTerm::Never
                    {
                        found = Some(term);
                        break;
                    }
                }
                found
            }
            _ => None,
        }))
    }
    /// The exact result of builtin primitive operators. Nominal operands
    /// need their selected operator's declared Output, never a guessed
    /// left-operand Type.
    fn builtin_binary_type(
        &mut self,
        db: &mut Database,
        node: NodeId,
        operation: SourceOperator,
    ) -> QueryPoll<Option<TyTerm>> {
        fn head(mut term: &TyTerm) -> &TyTerm {
            while let TyTerm::Ref(_, _, inner) = term {
                term = inner;
            }
            term
        }
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (Some(left), Some(right)) = (fact.operand(0), fact.operand(1)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let (Some(left_type), Some(right_type)) = (
            ready!(self.expression_type(db, left)),
            ready!(self.expression_type(db, right)),
        ) else {
            return QueryPoll::Ready(Ok(None));
        };
        let (left_type, right_type) = (head(&left_type), head(&right_type));
        let result = match (operation, left_type, right_type) {
            (SourceOperator::Arithmetic, TyTerm::Primitive(left), TyTerm::Primitive(right))
                if left == right
                    && (super::family(*left).is_some() || *left == PrimitiveType::Bool) =>
            {
                Some(*left)
            }
            (SourceOperator::Arithmetic, TyTerm::Primitive(primitive), other)
            | (SourceOperator::Arithmetic, other, TyTerm::Primitive(primitive))
                if *other == open() && super::family(*primitive).is_some() =>
            {
                let operand = if matches!(left_type, TyTerm::Primitive(_)) {
                    right
                } else {
                    left
                };
                ready!(self.literal_origin(db, operand))
                    .filter(|(_, floating)| Some(*floating) == super::family(*primitive))
                    .map(|_| *primitive)
            }
            (SourceOperator::Shift, TyTerm::Primitive(left), TyTerm::Primitive(right))
                if super::family(*left) == Some(false) && super::family(*right) == Some(false) =>
            {
                Some(*left)
            }
            (SourceOperator::Shift, TyTerm::Primitive(primitive), other)
                if super::family(*primitive) == Some(false) && *other == open() =>
            {
                ready!(self.literal_origin(db, right))
                    .filter(|(_, floating)| !*floating)
                    .map(|_| *primitive)
            }
            _ => None,
        };
        QueryPoll::Ready(Ok(result.map(TyTerm::Primitive)))
    }
    /// Actual indexed explicit Type bindings, shared by declared source
    /// parameter and result specialization. No separate operand reparse.
    pub(super) fn explicit_argument_bindings(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
    ) -> QueryPoll<BTreeMap<crate::traits::term::ParamKey, TyTerm>> {
        // Q44/Q47 currently describe indexed ordinary Call expressions.
        // Method/lang calls keep this optional goal open until their actual
        // declared-target proof joins the same query domain.
        if !self
            .fact(call)
            .is_ok_and(|fact| fact.kind() == FactKind::Call && fact.flag())
        {
            return QueryPoll::Ready(Ok(BTreeMap::new()));
        }
        // This is optional inference, not admission of an actual call.
        // Type-relative/binding callees can be selected at reached dispatch
        // while Q44 still has an explicitly unanswered signature goal. Do
        // not turn that state into Q47's strict admission refusal.
        let selected = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredCall(entry(self.key, call))
        ));
        let QueryValue::EnteredCall(goal) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint call Type query differs")));
        };
        if let Err(error) = goal.require_current(db) {
            return QueryPoll::Ready(Err(error));
        }
        if matches!(
            goal.selection(),
            crate::frontend::entered_call_goals::EnteredCallSelection::Pending(_)
                | crate::frontend::entered_call_goals::EnteredCallSelection::Binding(_)
                | crate::frontend::entered_call_goals::EnteredCallSelection::Value(_)
        ) {
            return QueryPoll::Ready(Ok(BTreeMap::new()));
        }
        let explicit = match db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedExplicitTypeArguments(entry(self.key, call)),
        ) {
            QueryPoll::Ready(Ok(explicit)) => explicit,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(BTreeMap::new()));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::PreparedExplicitTypeArguments(explicit) = explicit.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint explicit Type query differs")));
        };
        if explicit.definition() != definition {
            return QueryPoll::Ready(Err(fault("constraint explicit Type callee differs")));
        }
        let bound: BTreeMap<crate::traits::term::ParamKey, TyTerm> = explicit
            .rows()
            .iter()
            .map(|row| {
                (
                    crate::traits::term::ParamKey {
                        definition,
                        ordinal: row.ordinal(),
                    },
                    row.source().source().clone(),
                )
            })
            .collect();
        QueryPoll::Ready(Ok(bound))
    }
    /// A selected function's declared result with its own type parameters
    /// replaced by what the arguments' static Types state: a parameter named
    /// by a declared parameter Type is the matching part of the reached
    /// argument's Type (a method's receiver included). Parameters no
    /// argument states stay open.
    fn instantiate_result(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
        result: TyTerm,
    ) -> QueryPoll<TyTerm> {
        if !mentions_parameter(&result) {
            return QueryPoll::Ready(Ok(result));
        }
        // The shared binding and projection normalization of a declared goal
        // (`binding_constraint_projection.rs`): parameters replaced, then
        // `<T as Trait>::Name` of the bound Types normalized.
        self.instantiated_declared(db, call, definition, &result)
    }
    /// The arity of the tuple the pattern node being walked matches: the
    /// static Type of the value the root pattern `root` matches (a `let`
    /// initializer or `match` scrutinee), projected by the steps the walk
    /// has consumed since the root. Those are the root's projection less the
    /// current one, which is only a suffix of it while no nominal pattern
    /// has rewritten the steps; otherwise no arity is stated.
    pub(super) fn tuple_arity(
        &mut self,
        db: &mut Database,
        root: NodeId,
    ) -> QueryPoll<Option<usize>> {
        let Some(root_via) = self.root_via.clone() else {
            return QueryPoll::Ready(Ok(None));
        };
        let (all, rest) = (&root_via.steps, &self.via.steps);
        if !all.ends_with(rest) {
            return QueryPoll::Ready(Ok(None));
        }
        let consumed = all[..all.len() - rest.len()].to_vec();
        let plan = self.plan;
        let matched = plan.uses(root).iter().find_map(|edge| {
            let parent = plan.fact(edge.parent)?;
            match parent.kind() {
                FactKind::Let if edge.ordinal == 0 => parent.operand(1),
                FactKind::Match if edge.ordinal % 2 == 1 => parent.operand(0),
                _ => None,
            }
        });
        let Some(matched) = matched else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(matched) = ready!(self.expression_type(db, matched)) else {
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(
            match ready!(self.project_term(db, &matched, &consumed)) {
                Some(TyTerm::Tuple(items)) => Some(items.len()),
                Some(TyTerm::Primitive(PrimitiveType::Unit)) => Some(0),
                _ => None,
            },
        ))
    }
    /// The written annotation of a fact as its Type term.
    pub(super) fn annotation_term(
        &mut self,
        db: &mut Database,
        owner: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        let annotation = match self.fact(owner) {
            Ok(fact) => fact.annotation_node(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(annotation) = annotation else {
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(Some(ready!(self.written_term(db, annotation)))))
    }
    /// A record expression's or constructor call's selected nominal Type
    /// (its generic arguments are left to inference).
    fn constructed_type(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<Option<TyTerm>> {
        let constructed =
            ready!(self.constructor(db, super::nominal::ConstructorSite::Expression(node)));
        let Some(constructed) = constructed else {
            return QueryPoll::Ready(Ok(None));
        };
        // A type argument a field's operand states (a bare parameter field,
        // or a structure around it) is the matching part of that operand's
        // static Type; the others are left to inference.
        let plan = self.plan;
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut operands: Vec<(u32, NodeId)> = Vec::new();
        match fact.kind() {
            FactKind::Call => {
                for (ordinal, operand) in fact.operands().iter().enumerate().skip(1) {
                    if let (Some(operand), Ok(field)) = (operand, u32::try_from(ordinal - 1)) {
                        operands.push((field, *operand));
                    }
                }
            }
            FactKind::Record => {
                for (ordinal, row) in plan.record_fields(node).unwrap_or(&[]).iter().enumerate() {
                    if let (Some(field), Some(operand)) = (
                        self.written_field(&constructed, *row),
                        fact.operand(ordinal),
                    ) {
                        operands.push((field, operand));
                    }
                }
            }
            _ => {}
        }
        let mut arguments = vec![open(); constructed.type_parameter_count()];
        for (field, operand) in operands {
            let Some(actual) = ready!(self.expression_type(db, operand)) else {
                continue;
            };
            for (parameter, path) in constructed.field_parameters(field) {
                let Some(slot) = arguments.get_mut(parameter as usize) else {
                    continue;
                };
                if *slot != open() {
                    continue;
                }
                if let Some(part) = ready!(self.project_term(db, &actual, &path))
                    && part != open()
                {
                    // `slot` borrows `arguments`; re-index for the write.
                    arguments[parameter as usize] = part;
                }
            }
        }
        QueryPoll::Ready(Ok(Some(constructed.nominal_type_with(arguments))))
    }
    /// A path's selected local binding Type or own parameter Type.
    fn path_type(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<Option<TyTerm>> {
        let observed = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredValue(entry(self.key, node))
        ));
        let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("binding use query variant differs")));
        };
        let local = super::selected_local(observed);
        if super::selection_pending(observed) && local.is_none() {
            // A fieldless constructor named as a value (`None`) has its
            // nominal Type; any other pending goal states nothing.
            return self.constructed_type(db, node);
        }
        let Some(identity) = local else {
            return match observed.declaration() {
                Some(declaration) => {
                    let identity = declaration.identity();
                    match ready!(self.declared_item_type(db, observed, identity)) {
                        Some(term) => QueryPoll::Ready(Ok(Some(term))),
                        // A unit struct named as a value is a value of its
                        // own declaration's Type.
                        None => self.constructed_type(db, node),
                    }
                }
                None => QueryPoll::Ready(Ok(None)),
            };
        };
        match ready!(self.binding_pattern(db, node, identity)) {
            Some(pattern) => self.binding_type(db, pattern),
            None => self.own_parameter_term(db, node),
        }
    }
    /// A constant's or static's declared Type: the written Type of its
    /// header (`const NAME: Type = ..`, `static NAME: Type = ..`, an
    /// associated `const NAME: Type`), resolved in its declaration scope as
    /// any written Type is. Consume the actual selected declaration and the
    /// shared initializer result goal for original and generated source.
    pub(super) fn declared_item_type(
        &mut self,
        db: &mut Database,
        goal: &crate::frontend::source_value_goals::EnteredValueGoal,
        identity: Digest,
    ) -> QueryPoll<Option<TyTerm>> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        use crate::db::dependency::{InputKey, InputValue};
        use ra_ap_syntax::{AstNode, ast};
        let Some(selected) = goal
            .declaration()
            .filter(|selected| selected.identity() == identity)
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let source = db.input(&InputKey::DefinitionSource(identity));
        let header = db.input(&InputKey::DefinitionHeader(identity));
        let context = db.input(&InputKey::DeclarationContext(identity));
        let (Some(source), Some(header), Some(context)) = (source, header, context) else {
            return QueryPoll::Ready(Ok(None));
        };
        let (
            InputValue::DefinitionSource(source),
            InputValue::DefinitionHeader(header),
            InputValue::DeclarationContext(context),
        ) = (source.as_ref(), header.as_ref(), context.as_ref())
        else {
            return QueryPoll::Ready(Err(fault("selected constant Type input variants differ")));
        };
        if source != selected.source()
            || context != selected.context()
            || crate::core::digest::StableEncode::fingerprint(header) != selected.header()
        {
            return QueryPoll::Ready(Err(fault(
                "selected constant Type source/header/context differs",
            )));
        }
        let parsed = ready!(db.fetch(&SyntaxProvider, QueryKey::ParseFile(source.clone())));
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("selected constant Type parse variant differs")));
        };
        match parsed.syntax_at(context.range, |node| {
            ast::Const::can_cast(node.kind()) || ast::Static::can_cast(node.kind())
        }) {
            Ok(Some(_)) => {}
            Ok(None) => return QueryPoll::Ready(Ok(None)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // The existing signature query also owns actual Const/Static result
        // annotations. Its local and module paths retain their real Type
        // occurrences; do not fabricate a function or reparse header text.
        match ready!(crate::frontend::source_return_goals::prepare(
            db,
            environment.names(),
            &sources,
            identity,
        )) {
            crate::frontend::source_return_goals::ReturnTypeGoal::Ready(result) => {
                QueryPoll::Ready(Ok(Some(result.source().clone())))
            }
            _ => QueryPoll::Ready(Ok(None)),
        }
    }
    /// A local's Type: its owner pattern's matched Type (a `let`
    /// annotation or initializer, or a `match` scrutinee) projected to the
    /// binding's position.
    fn binding_type(&mut self, db: &mut Database, pattern: NodeId) -> QueryPoll<Option<TyTerm>> {
        let plan = self.plan;
        let Some(occurrence) = plan.occurrence(pattern) else {
            // A closure parameter: its written Type, else the parameter Type
            // the closure is passed for.
            return self.closure_parameter_type(db, pattern);
        };
        let (matched, skipped) = ready!(self.root_matched_type(db, occurrence.root));
        let Some(matched) = matched else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some((steps, by_reference)) =
            ready!(self.occurrence_steps(db, occurrence, skipped, Some(matched.clone())))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let projected = ready!(self.project_term(db, &matched, &steps[skipped..]));
        QueryPoll::Ready(Ok(
            projected.map(|term| if by_reference { borrowed(term) } else { term })
        ))
    }
    /// The method a method call selects: the dispatcher's probe rule over
    /// the receiver's static Type.
    pub(super) fn method_function(
        &mut self,
        db: &mut Database,
        call: NodeId,
    ) -> QueryPoll<Option<Digest>> {
        let fact = match self.fact(call) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (Some(receiver), Some(name)) = (
            fact.operand(0),
            fact.member().and_then(|row| self.plan.member(row)),
        ) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(receiver) = ready!(self.expression_type(db, receiver)) else {
            return QueryPoll::Ready(Ok(None));
        };
        // An unsuffixed literal receiver (`3.b()`): the method of the one
        // trait its family implements that provides `name`.
        if receiver == open()
            && let Some(receiver_node) = fact.operand(0)
        {
            let name = name.to_owned();
            return self.numeric_receiver_method(db, receiver_node, &name);
        }
        let Some(self_type) = probe_self(&receiver) else {
            // A receiver of the function's own type parameter: its bounds'
            // declared method.
            let mut parameter = &receiver;
            while let TyTerm::Ref(_, _, inner) = parameter {
                parameter = inner;
            }
            if let TyTerm::Param(key) = parameter {
                let key = *key;
                let name = name.to_owned();
                return self.bound_method(db, &key, &name);
            }
            // A trait object: its principal trait declares the method.
            if let TyTerm::Dyn(principal) = parameter {
                let principal = principal.0;
                let name = name.to_owned();
                return self.dyn_method(db, principal, &name);
            }
            return QueryPoll::Ready(Ok(None));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let reference_sources: Vec<_> = environment.sources().iter().collect();
        if let Some(selected) = ready!(crate::frontend::reference_methods::probe(
            db,
            &members,
            environment.names(),
            &reference_sources,
            &receiver,
            name
        )) {
            return QueryPoll::Ready(Ok(match selected.alternatives.functions.as_slice() {
                [function] => Some(*function),
                _ => None,
            }));
        }
        // No candidate on the static receiver Type itself (an autoderef or
        // unindexed trait method) leaves the selection open, as do several
        // candidates (impls of one trait that differ by the trait's
        // arguments, which rustc selects by the argument types): the
        // dispatcher selects at the reached call. Ambiguity rustc would
        // report is not re-enforced here.
        match probe_methods(&members, self_type, name) {
            Ok(alternatives) => match alternatives.functions.as_slice() {
                [function] => QueryPoll::Ready(Ok(Some(*function))),
                _ => QueryPoll::Ready(Ok(None)),
            },
            // No candidate on the static receiver Type: the probe continues
            // through the receiver's overloaded `Deref` targets, as the
            // dispatcher's does.
            Err(error) if super::not_closed(&error) => {
                self.deref_method(db, &members, &receiver, name)
            }
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// The method the probe selects after the receiver's overloaded `Deref`:
    /// the growable-array lang item dereferences to `[T]` with the same
    /// element projection this service already gives it (`Step::Element`),
    /// so the receiver relations of the slice method hold on the `Vec`
    /// receiver. Another type's `Deref` target is not projected by the
    /// receiver's steps; its selection stays open for the dispatcher.
    fn deref_method(
        &mut self,
        db: &mut Database,
        members: &crate::frontend::member_index::MemberIndex,
        receiver: &TyTerm,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let mut term = receiver;
        while let TyTerm::Ref(_, _, inner) = term {
            term = inner;
        }
        // `Box<dyn Trait>` dereferences (builtin) to the trait object, whose
        // principal trait declares the method.
        if let Some(TyTerm::Dyn(principal)) = ready!(self.box_contents(db, term)) {
            let name = name.to_owned();
            return self.dyn_method(db, principal.0, &name);
        }
        let Some(element) = ready!(self.vec_elements(db, term)) else {
            return self.overloaded_deref_method(db, members, term, name);
        };
        let target = TyTerm::Slice(Box::new(element));
        let Some(bucket) = probe_self(&target) else {
            return QueryPoll::Ready(Ok(None));
        };
        match probe_methods(members, bucket, name) {
            Ok(alternatives) => QueryPoll::Ready(Ok(match alternatives.functions.as_slice() {
                [function] => Some(*function),
                _ => None,
            })),
            Err(error) if super::not_closed(&error) => QueryPoll::Ready(Ok(None)),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// The method the probe selects after the receiver's overloaded `Deref`
    /// chain (`String` to `str`, `Rc<T>` to `T`, a user smart pointer to its
    /// `Target`), to rustc's autoderef `recursion_limit`. The receiver's
    /// parts map onto the selected method's declared receiver Type by the
    /// same dereferences (`adjust_receiver`).
    fn overloaded_deref_method(
        &mut self,
        db: &mut Database,
        members: &crate::frontend::member_index::MemberIndex,
        receiver: &TyTerm,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let mut current = receiver.clone();
        for _ in 0..super::RECURSION_LIMIT {
            if !matches!(current, TyTerm::Nominal(..)) {
                break;
            }
            let target = match ready!(self.deref_target(db, &current)) {
                Some(target) => Some(target),
                // An arbitrary-self-types step names the next type only.
                None => ready!(self.receiver_target(db, &current)),
            };
            let Some(mut target) = target else {
                break;
            };
            while let TyTerm::Ref(_, _, inner) = target {
                target = *inner;
            }
            if let Some(bucket) = probe_self(&target) {
                match probe_methods(members, bucket, name) {
                    Ok(alternatives) => {
                        if let [function] = alternatives.functions.as_slice() {
                            return QueryPoll::Ready(Ok(Some(*function)));
                        }
                    }
                    Err(error) if super::not_closed(&error) => {}
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            current = target;
        }
        QueryPoll::Ready(Ok(None))
    }
    /// The selected method's declared signature.
    pub(super) fn method_signature(
        &mut self,
        db: &mut Database,
        function: Digest,
    ) -> QueryPoll<Option<std::sync::Arc<QueryValue>>> {
        match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(function)) {
            QueryPoll::Ready(Ok(value)) => QueryPoll::Ready(Ok(Some(value.value))),
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// A method call's value is the selected method's declared result.
    pub(super) fn method_inward(&mut self, db: &mut Database, call: NodeId) -> QueryPoll<()> {
        match ready!(self.method_function(db, call)) {
            Some(function) => self.result(db, call, function),
            None => {
                self.defer(call, ConstraintDemand::MethodSelection);
                QueryPoll::Ready(Ok(()))
            }
        }
    }
    /// A method call operand: an argument takes its declared parameter; a
    /// receiver holding the variable inside its structure is related only
    /// when the selected method's other parameters or result mention the
    /// impl's type parameters (`[T]::len` relates nothing).
    pub(super) fn method_outward(
        &mut self,
        db: &mut Database,
        call: NodeId,
        ordinal: u32,
    ) -> QueryPoll<()> {
        if ordinal == 0 && self.via.is_scalar() && !self.open {
            // The method selected depends on the variable's own Type: rustc's
            // trait candidates for a numeric inference variable.
            return self.numeric_receiver(db, call);
        }
        let Some(function) = ready!(self.method_function(db, call)) else {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(signature) = ready!(self.method_signature(db, function)) else {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint method signature differs")));
        };
        let receiver = signature
            .parameters
            .first()
            .is_some_and(|parameter| parameter.receiver);
        if !receiver {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        }
        if ordinal > 0 {
            return self.declared_parameter(db, call, function, ordinal);
        }
        let parameters = u32::try_from(signature.parameters.len()).unwrap_or(u32::MAX);
        // The selected impl's self type states the receiver's own parts
        // (`impl GenericFoo<u32> { fn bar(self) }`): its declared receiver
        // Type, under the receiver expression's auto-ref/deref.
        let saved = (self.pending, self.unrecorded);
        let declared = ready!(self.parameter_term(db, call, function, 0));
        (self.pending, self.unrecorded) = saved;
        if let Some(mut declared) = declared {
            while let TyTerm::Ref(_, _, inner) = declared {
                declared = *inner;
            }
            let mut steps: &[Step] = &self.via.steps;
            while let Some((Step::Deref | Step::AutoDeref, rest)) = steps.split_first() {
                steps = rest;
            }
            let steps = steps.to_vec();
            // The receiver may have been dereferenced to the declared self
            // type by the probe (`Vec<T>` to `[T]`, `Rc<T>` to `T`).
            let Some(candidates) = ready!(self.adjust_receiver(db, call, &declared, steps)) else {
                self.defer(call, ConstraintDemand::MethodSelection);
                return QueryPoll::Ready(Ok(()));
            };
            let mut handled = false;
            for steps in candidates {
                match ready!(self.project_parametric(db, &declared, &steps)) {
                    super::generics::TermProjection::Leaf(TyTerm::Primitive(primitive)) => {
                        ready!(QueryPoll::Ready(self.constrain(call, primitive)));
                        handled = true;
                    }
                    // The impl's self type states the part (`impl W<u8> { .. }`).
                    super::generics::TermProjection::Leaf(term)
                        if self.open && crate::frontend::call_contracts::closed_term(&term) =>
                    {
                        self.candidate(call, term);
                        handled = true;
                    }
                    // The part is the impl's own type parameter: every other
                    // position of it in the method's signature (the arguments and
                    // the result) holds the same Type.
                    super::generics::TermProjection::Parameter(key, remaining)
                        if key.definition == function =>
                    {
                        ready!(self.relate_parameter(db, call, function, key, remaining));
                        handled = true;
                    }
                    super::generics::TermProjection::Leaf(term)
                        if self.open && mentions_parameter(&term) =>
                    {
                        ready!(self.relate_signature_type(db, call, function, &term, &[]));
                        handled = true;
                    }
                    _ => {}
                }
            }
            if handled {
                return QueryPoll::Ready(Ok(()));
            }
        }
        // The declared goals, read without recording this component's own
        // pending demands (a missing goal is a relation not stated).
        let saved = (self.pending, self.unrecorded);
        let mut others_relate = false;
        for ordinal in 1..parameters {
            match ready!(self.parameter_term(db, call, function, ordinal)) {
                Some(term) if !mentions_parameter(&term) => {}
                _ => others_relate = true,
            }
        }
        if !others_relate {
            match ready!(self.result_term(db, call, function)) {
                Some(term) if !mentions_parameter(&term) => {}
                _ => others_relate = true,
            }
        }
        (self.pending, self.unrecorded) = saved;
        if others_relate {
            self.defer(call, ConstraintDemand::MethodSelection);
        }
        QueryPoll::Ready(Ok(()))
    }
    /// An index operand: rustc's builtin array/slice indexing takes `usize`.
    /// Another base (a `Vec`, a map, a custom `Index` impl) keeps the
    /// existing held route.
    pub(super) fn index_operand(&mut self, db: &mut Database, index: NodeId) -> QueryPoll<()> {
        let base = match self.fact(index) {
            Ok(fact) => fact.operand(0),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let base = match base {
            Some(base) => ready!(self.expression_type(db, base)),
            None => None,
        };
        let mut base = base.as_ref();
        while let Some(TyTerm::Ref(_, _, inner)) = base {
            base = Some(inner);
        }
        // A `Vec<T>` place indexes its element cells like an array's.
        let vector = match base {
            Some(term) => ready!(self.vec_elements(db, term)).is_some(),
            None => false,
        };
        match base {
            Some(TyTerm::Array(..) | TyTerm::Slice(_)) if self.via.is_scalar() => {
                ready!(QueryPoll::Ready(
                    self.constrain(index, PrimitiveType::Usize)
                ));
            }
            Some(_) if vector && self.via.is_scalar() => {
                ready!(QueryPoll::Ready(
                    self.constrain(index, PrimitiveType::Usize)
                ));
            }
            // A user type with one `Index<Idx>` impl: the operand is that
            // impl's `index` argument, as a method call's is.
            Some(TyTerm::Nominal(identity, _)) if !vector => {
                let identity = *identity;
                match ready!(self.single_index_method(db, identity)) {
                    Some(function) => ready!(self.declared_parameter(db, index, function, 1)),
                    None => self.mark_unrecorded(),
                }
            }
            _ => self.mark_unrecorded(),
        }
        QueryPoll::Ready(Ok(()))
    }
    /// The `index` method of the one `Index<Idx>` impl of the nominal type
    /// `identity`; several impls (the operand selects) state nothing here.
    fn single_index_method(
        &mut self,
        db: &mut Database,
        identity: Digest,
    ) -> QueryPoll<Option<Digest>> {
        let Some(trait_key) = ready!(self.core_trait(db, "ops", "Index")) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut functions = Vec::new();
        for (_, function) in members.trait_method(Ok(identity), trait_key, "index") {
            if !functions.contains(&function) {
                functions.push(function);
            }
        }
        QueryPoll::Ready(Ok(match functions.as_slice() {
            [function] => Some(*function),
            _ => None,
        }))
    }
}
