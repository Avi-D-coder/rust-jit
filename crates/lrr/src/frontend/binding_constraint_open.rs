//! Open type arguments in the source constraint service.
//!
//! A selected function's (or fieldless constructor's) result may name type
//! parameters that none of its arguments fix: `impl<T> W<T> { fn new() ->
//! W<T> }`, `None`, `Vec::new()`. rustc leaves them as inference variables
//! that later code decides: a method call whose declared signature relates
//! the receiver's parts to its parameters, an assignment, a typed
//! parameter, a comparison, a constructor field. This module starts the
//! same component solver the numeric literal demand uses from the call or
//! constructor expression itself, with the open argument's projection
//! (`TypeArgument(k)`) as the member's steps; every relation and
//! declared-signature service is the numeric demand's own. The one
//! difference is the answer: a closed Type term (the earliest relation in
//! source order decides, as rustc's checking order does), or the
//! integer/float fallback when only unsuffixed literals reached the variable
//! and the component is closed. A Type built around another constructed
//! value's own parameters (`Box<List<T>>`) is decided by the same demand
//! started at that value. Anything else stays an unresolved goal: the call
//! keeps its explicit unsupported diagnostic.
//!
//! The answer is a written Type that the expression's own recipe states
//! first, exactly as a parent's `SetHint` of its result would
//! (`db/binding_constraint_thunk.rs` for value paths and unclaimed calls,
//! `PreparedExplicitTypeArguments::inferred_result` for source calls).
use super::{
    BindingConstraintState, Component, PreparedBindingConstraints, Step, Via, fault,
    generics::parameter_paths,
};
use crate::{
    core::digest::Digest,
    db::{memo::Database, poll::QueryPoll, value::QueryValue},
    ir::{
        op::PrimitiveType,
        pc::{NodeId, PcKey},
        written_type::WrittenType,
    },
    syntax::source_constraints::{FactKind, SourceConstraintPlan},
    traits::term::{GenericArg, ParamKey, TyTerm},
};
use std::{collections::BTreeMap, sync::Arc};

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// An initializer's declared Type with the type parameters its arguments do
/// not fix left as `Param` variables of the selected declaration.
pub(super) struct OpenShape {
    term: TyTerm,
    keys: Vec<ParamKey>,
    /// The selected function whose generic parameters `keys` are, when the
    /// shape is a function's result (a constructor's are not turbofish).
    callee: Option<Digest>,
}

/// Every `Param` of `definition` named anywhere in `term`.
fn parameters_of(term: &TyTerm, definition: Digest, out: &mut Vec<ParamKey>) {
    match term {
        TyTerm::Param(key) if key.definition == definition => {
            if !out.contains(key) {
                out.push(*key);
            }
        }
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            parameters_of(inner, definition, out)
        }
        TyTerm::Tuple(members) => {
            for member in members {
                parameters_of(member, definition, out);
            }
        }
        TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => {
            for arg in args {
                if let GenericArg::Type(inner) = arg {
                    parameters_of(inner, definition, out);
                }
            }
        }
        TyTerm::Projection(projection) => {
            parameters_of(&projection.trait_ref.self_ty, definition, out);
            for arg in projection.arguments() {
                if let GenericArg::Type(inner) = arg {
                    parameters_of(inner, definition, out);
                }
            }
        }
        _ => {}
    }
}

/// `term` with each parameter of `solved` replaced by its Type.
fn substitute(term: &TyTerm, solved: &BTreeMap<ParamKey, TyTerm>) -> TyTerm {
    match term {
        TyTerm::Param(key) => solved.get(key).cloned().unwrap_or_else(|| term.clone()),
        TyTerm::Ref(region, mutable, inner) => TyTerm::Ref(
            region.clone(),
            *mutable,
            Box::new(substitute(inner, solved)),
        ),
        TyTerm::Slice(inner) => TyTerm::Slice(Box::new(substitute(inner, solved))),
        TyTerm::Array(inner, len) => {
            TyTerm::Array(Box::new(substitute(inner, solved)), len.clone())
        }
        TyTerm::Tuple(members) => TyTerm::Tuple(
            members
                .iter()
                .map(|member| substitute(member, solved))
                .collect(),
        ),
        TyTerm::Nominal(identity, args) => TyTerm::Nominal(
            *identity,
            args.iter()
                .map(|arg| match arg {
                    GenericArg::Type(inner) => GenericArg::Type(substitute(inner, solved)),
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

impl<'a> Component<'a> {
    /// The open type parameters of an initializer expression, or `None`
    /// when its Type is fixed by its arguments or is not stated by the
    /// common services. Reads declared goals only; it records no relation.
    pub(super) fn open_shape(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<OpenShape>> {
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match fact.kind() {
            FactKind::Parenthesis => match fact.operand(0) {
                Some(inner) => self.open_shape(db, inner),
                None => QueryPoll::Ready(Ok(None)),
            },
            // A fieldless constructor named as a value (`None`): every type
            // argument is open.
            FactKind::Path => {
                let constructed =
                    ready!(self.constructor(db, super::nominal::ConstructorSite::Expression(node)));
                QueryPoll::Ready(Ok(constructed
                    .filter(|constructed| constructed.type_parameter_count() > 0)
                    .map(|constructed| {
                        let (term, keys) = constructed.open_type();
                        OpenShape {
                            term,
                            keys,
                            callee: None,
                        }
                    })))
            }
            FactKind::Call => {
                // Explicit generic arguments (`f::<u8>()`, `Vec::<u8>::new()`)
                // are the call's own explicit Type goal.
                if fact.flag() {
                    return QueryPoll::Ready(Ok(None));
                }
                // A constructor call's arguments decide the type arguments
                // its fields name; the others (`E::B(3)` of `enum E<T> {
                // A(T), B(u8) }`) are open.
                if let Some(constructed) =
                    ready!(self.constructor(db, super::nominal::ConstructorSite::Expression(node)))
                {
                    let (term, keys) = constructed.open_type();
                    let keys: Vec<_> = keys
                        .into_iter()
                        .filter(|key| !constructed.names_parameter(key.ordinal))
                        .collect();
                    return QueryPoll::Ready(Ok((!keys.is_empty()).then_some(OpenShape {
                        term,
                        keys,
                        callee: None,
                    })));
                }
                match ready!(self.callee(db, node)) {
                    Some(super::Callee::Source(definition)) => {
                        self.function_open_shape(db, node, definition)
                    }
                    _ => QueryPoll::Ready(Ok(None)),
                }
            }
            FactKind::MethodCall => match ready!(self.method_function(db, node)) {
                Some(function) => self.function_open_shape(db, node, function),
                None => QueryPoll::Ready(Ok(None)),
            },
            _ => QueryPoll::Ready(Ok(None)),
        }
    }

    /// The selected function's declared result with the type parameters
    /// that neither a declared parameter Type, an explicit argument nor a
    /// written bound relates left open.
    fn function_open_shape(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
    ) -> QueryPoll<Option<OpenShape>> {
        let Some(result) = ready!(self.quiet_result_term(db, call, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut open = Vec::new();
        parameters_of(&result, definition, &mut open);
        if open.is_empty() {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(signature) = ready!(self.method_signature(db, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Err(fault("open constraint signature differs")));
        };
        // A parameter named by an argument's declared Type is decided by
        // the reached argument.
        let mut fixed = Vec::new();
        for ordinal in 0..signature.parameters.len() {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return QueryPoll::Ready(Ok(None));
            };
            match ready!(self.quiet_parameter_term(db, call, definition, ordinal)) {
                Some(term) => parameters_of(&term, definition, &mut fixed),
                // An unreadable parameter may fix any of them.
                None => return QueryPoll::Ready(Ok(None)),
            }
        }
        // A parameter a bound relates through an associated type
        // (`I: Iterator<Item = U>`) is decided by the bound's other side.
        let related: Vec<&str> = signature
            .generics
            .iter()
            .filter(|parameter| {
                signature
                    .generics
                    .iter()
                    .map(|other| other.bounds.as_slice())
                    .chain(std::iter::once(signature.predicates.as_slice()))
                    .any(|tokens| {
                        tokens.iter().any(|token| token.text == "=")
                            && tokens.iter().any(|token| token.text == parameter.name)
                    })
            })
            .map(|parameter| parameter.name.as_str())
            .collect();
        open.retain(|key| {
            !fixed.contains(key)
                && !signature.generics.iter().any(|parameter| {
                    parameter.ordinal == key.ordinal && related.contains(&parameter.name.as_str())
                })
        });
        if open.is_empty() {
            return QueryPoll::Ready(Ok(None));
        }
        QueryPoll::Ready(Ok(Some(OpenShape {
            term: result,
            keys: open,
            callee: Some(definition),
        })))
    }

    /// The closed Type `node`'s own static Type states at the current
    /// member's projection, if any. The static Type reads the common
    /// services only and records no relation; an unstated or still-open
    /// part states nothing.
    pub(super) fn static_candidate(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        let Some(term) = ready!(self.expression_type(db, node)) else {
            return QueryPoll::Ready(Ok(()));
        };
        let steps = self.via.steps.clone();
        let saved = (self.pending, self.unrecorded);
        let projected = self.project_term(db, &term, &steps);
        (self.pending, self.unrecorded) = saved;
        if let Some(projected) = ready!(projected)
            && crate::frontend::call_contracts::closed_term(&projected)
        {
            self.candidate(node, projected);
        }
        QueryPoll::Ready(Ok(()))
    }

    /// The Types later code states for the open variable, earliest in
    /// source order first, then the integer/float fallback of a closed
    /// component of unsuffixed literals (rustc applies it only when no
    /// relation decided the variable).
    fn open_answers(&self) -> Vec<TyTerm> {
        let mut answers: Vec<TyTerm> = self.candidates.values().cloned().collect();
        if answers.is_empty()
            && let (Some(floating), None, false) =
                (self.literal_family, self.pending, self.unrecorded)
        {
            answers.push(TyTerm::Primitive(if floating {
                PrimitiveType::F64
            } else {
                PrimitiveType::I32
            }));
        }
        answers
    }
}

/// The first of a solved component's answers whose placeholder parameters
/// (the Types of other constructed values) all have answers of their own.
/// A placeholder is decided by the same demand, started at the constructed
/// expression's type argument; one already being decided has no answer.
fn closed_answer(
    db: &mut Database,
    key: &PcKey,
    plan: &SourceConstraintPlan,
    component: &Component<'_>,
    visiting: &mut Vec<(NodeId, u32)>,
) -> QueryPoll<Option<TyTerm>> {
    'answers: for answer in component.open_answers() {
        let mut dependents = Vec::new();
        parameters_of_all(&answer, &component.dependents, &mut dependents);
        let mut solved = BTreeMap::new();
        for placeholder in dependents {
            let Some(&(node, ordinal)) = component.dependents.get(&placeholder) else {
                continue 'answers;
            };
            if visiting.contains(&(node, ordinal)) {
                continue 'answers;
            }
            visiting.push((node, ordinal));
            let mut nested = Component::new(key, plan, false);
            nested.open = true;
            nested.deciding = component.deciding.clone();
            nested.expression_via(
                Some(node),
                Via {
                    steps: vec![Step::TypeArgument(ordinal)],
                    strict: false,
                },
            );
            let outcome = match nested.solve(db) {
                QueryPoll::Ready(Ok(())) => closed_answer(db, key, plan, &nested, visiting),
                QueryPoll::Ready(Err(_)) => QueryPoll::Ready(Ok(None)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
            visiting.pop();
            match ready!(outcome) {
                Some(term) => {
                    solved.insert(placeholder, term);
                }
                None => continue 'answers,
            }
        }
        let term = substitute(&answer, &solved);
        if crate::frontend::call_contracts::closed_term(&term) {
            return QueryPoll::Ready(Ok(Some(term)));
        }
    }
    QueryPoll::Ready(Ok(None))
}
/// Every placeholder parameter of `dependents` that `term` names.
fn parameters_of_all(
    term: &TyTerm,
    dependents: &BTreeMap<ParamKey, (NodeId, u32)>,
    out: &mut Vec<ParamKey>,
) {
    for key in dependents.keys() {
        if !out.contains(key) && names_parameter(term, key) {
            out.push(*key);
        }
    }
}
fn names_parameter(term: &TyTerm, key: &ParamKey) -> bool {
    let mut found = Vec::new();
    parameters_of(term, key.definition, &mut found);
    found.contains(key)
}

/// Each open parameter of `shape`, the declared Type of the call or
/// constructor `node`, decided by the expression's later uses. `deciding`
/// holds the expressions whose Types are being decided further up: one of
/// them cannot be asked again while its own answer is awaited.
fn solve_shape(
    db: &mut Database,
    key: &PcKey,
    plan: &SourceConstraintPlan,
    node: NodeId,
    shape: &OpenShape,
    deciding: &[NodeId],
) -> QueryPoll<Option<BTreeMap<ParamKey, TyTerm>>> {
    let mut solved = BTreeMap::new();
    for open in &shape.keys {
        let Some(steps) = parameter_paths(&shape.term, open).into_iter().next() else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut component = Component::new(key, plan, false);
        component.open = true;
        component.deciding = deciding.to_vec();
        component.deciding.push(node);
        let via = Via {
            strict: steps.is_empty(),
            steps,
        };
        component.expression_via(Some(node), via);
        match component.solve(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(_)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let answer = ready!(closed_answer(db, key, plan, &component, &mut Vec::new()));
        let Some(answer) = answer else {
            return QueryPoll::Ready(Ok(None));
        };
        solved.insert(*open, answer);
    }
    QueryPoll::Ready(Ok(Some(solved)))
}

impl<'a> Component<'a> {
    /// `term`, the static Type of the call or constructor `node`, with the
    /// type arguments later code decided for it when `term` still names
    /// open parameters: the same demand an expression of that shape starts,
    /// answered for this node. A node already being decided further up, or
    /// one the demand cannot decide, keeps `term`.
    pub(super) fn decided(
        &mut self,
        db: &mut Database,
        node: NodeId,
        term: TyTerm,
    ) -> QueryPoll<TyTerm> {
        if !self.open
            || !term_names_parameter(&term)
            || self.deciding.contains(&node)
            || self.deciding.len() > super::RECURSION_LIMIT
        {
            return QueryPoll::Ready(Ok(term));
        }
        let mut probe = Component::new(self.key, self.plan, false);
        probe.open = true;
        probe.deciding = self.deciding.clone();
        probe.deciding.push(node);
        let shape = match probe.open_shape(db, node) {
            QueryPoll::Ready(Ok(Some(shape))) => shape,
            QueryPoll::Ready(Ok(None)) | QueryPoll::Ready(Err(_)) => {
                return QueryPoll::Ready(Ok(term));
            }
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let deciding = self.deciding.clone();
        let (key, plan) = (self.key, self.plan);
        let solved = match solve_shape(db, key, plan, node, &shape, &deciding) {
            QueryPoll::Ready(Ok(Some(solved))) => solved,
            QueryPoll::Ready(Ok(None)) | QueryPoll::Ready(Err(_)) => {
                return QueryPoll::Ready(Ok(term));
            }
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let decided = substitute(&term, &solved);
        QueryPoll::Ready(Ok(
            if crate::frontend::call_contracts::closed_term(&decided) {
                decided
            } else {
                term
            },
        ))
    }
}
/// Whether `term` names any generic parameter.
fn term_names_parameter(term: &TyTerm) -> bool {
    match term {
        TyTerm::Param(_) => true,
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            term_names_parameter(inner)
        }
        TyTerm::Tuple(members) => members.iter().any(term_names_parameter),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(inner) => term_names_parameter(inner),
            _ => false,
        }),
        _ => false,
    }
}

/// An expression whose declared Type names open type parameters (a call
/// whose arguments do not fix them, a fieldless generic constructor): solve
/// each from the value's later uses and publish the expression's Type as a
/// written hint. Whatever the common services cannot state keeps the
/// existing route and its explicit diagnostic.
pub(super) fn prepare_expression(
    db: &mut Database,
    key: &PcKey,
    plan: &SourceConstraintPlan,
    mut result: PreparedBindingConstraints,
) -> QueryPoll<PreparedBindingConstraints> {
    let node = NodeId(key.node);
    let mut probe = Component::new(key, plan, false);
    probe.open = true;
    probe.deciding.push(node);
    // Detection is speculative: an expression it cannot state has no open
    // arguments as far as this demand is concerned.
    let shape = match probe.open_shape(db, node) {
        QueryPoll::Ready(Ok(Some(shape))) => shape,
        QueryPoll::Ready(Ok(None)) | QueryPoll::Ready(Err(_)) => {
            return QueryPoll::Ready(Ok(result));
        }
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let solved = ready!(solve_shape(db, key, plan, node, &shape, &[]));
    let Some(solved) = solved else {
        return QueryPoll::Ready(Ok(result));
    };
    let closed = substitute(&shape.term, &solved);
    let environment = match probe.environment(db) {
        QueryPoll::Ready(Ok(Some(environment))) => environment,
        QueryPoll::Ready(Ok(None)) | QueryPoll::Ready(Err(_)) => {
            return QueryPoll::Ready(Ok(result));
        }
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let sources: Vec<_> = environment.sources().iter().collect();
    // A part the arguments fix stays an inference leaf of the written hint.
    let written = match crate::frontend::written_terms::written_template(
        db,
        &sources,
        &closed,
        Digest::default(),
    ) {
        QueryPoll::Ready(Ok(Some(written))) if !matches!(written, WrittenType::Infer) => {
            Some(written)
        }
        QueryPoll::Ready(Ok(_)) | QueryPoll::Ready(Err(_)) => None,
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    // The function's own generic parameters, as a written turbofish would
    // state them: closed written Types by callee parameter ordinal.
    if let Some(definition) = shape.callee {
        let mut arguments = Vec::with_capacity(shape.keys.len());
        for open in &shape.keys {
            let Some(solved) = solved.get(open) else {
                continue;
            };
            match crate::frontend::written_terms::closed_written_type(db, &sources, solved) {
                QueryPoll::Ready(Ok(Some(written))) => {
                    arguments.push((open.ordinal, Arc::new(written)))
                }
                QueryPoll::Ready(Ok(None)) | QueryPoll::Ready(Err(_)) => {}
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        if !arguments.is_empty() {
            result.arguments = Some(Arc::new(crate::ir::recipe::InferredArguments {
                definition,
                arguments,
            }));
        }
    }
    // Either answer is enough: the result Type as a hint, and the callee's
    // own parameters as explicit arguments.
    if written.is_none() && result.arguments.is_none() {
        return QueryPoll::Ready(Ok(result));
    }
    result.written = written.map(Arc::new);
    result.state = BindingConstraintState::ResolvedType;
    QueryPoll::Ready(Ok(result))
}
