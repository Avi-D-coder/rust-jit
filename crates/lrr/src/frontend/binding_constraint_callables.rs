//! Callable values typed by `Fn*(..) -> R` sugar in the source numeric
//! constraint service.
//!
//! A value whose Type is a generic parameter bounded by `F: FnMut(A, B) -> R`
//! (also `where F: ..`), an argument-position `impl Fn(A) -> R`, a trait
//! object `&dyn Fn(A) -> R` or a function pointer `fn(A) -> R` is called with
//! arguments of the written `A, B` and produces the written `R`. The same
//! signature relates a closure written where such a parameter is passed: the
//! closure's parameters hold `A, B` and its body value holds `R` (rustc's
//! closure signature deduction from the expected `Fn*` bound), so a generic
//! parameter of the callee that these Types name is related to the closure's
//! parameters and body, exactly as it is to the other arguments of the call.
//!
//! The sugar's Types are resolved by the shared written-Type service in the
//! scope of the declaration that wrote them; nothing here guesses a Type. A
//! part the service cannot state stays an unanswered goal.
use super::{
    Callee, Component, ConstraintDemand, Step, Via, fault, generics::parameter_paths,
    nominal::ConstructorSite, not_closed,
};
use crate::{
    core::digest::Digest,
    db::{memo::Database, poll::QueryPoll, value::QueryValue},
    ir::{op::PrimitiveType, pc::NodeId},
    syntax::{
        ast_adapter::TokenRecipe,
        signatures::{FunctionSignature, GenericKind},
        source_constraints::FactKind,
    },
    traits::term::{ParamKey, TyTerm},
};
use std::sync::Arc;

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// A callable Type's signature as its `Fn*(..) -> R` sugar states it. A part
/// the shared Type service cannot state is `None`.
#[derive(Debug)]
pub(super) struct FnBound {
    pub(super) params: Vec<Option<TyTerm>>,
    pub(super) result: Option<TyTerm>,
}

/// The argument and result token ranges of the first `Fn`/`FnMut`/`FnOnce`
/// sugar or `fn` pointer in `tokens`; an absent result is the unit Type.
pub(super) fn fn_sugar(tokens: &[TokenRecipe]) -> Option<(Vec<&[TokenRecipe]>, &[TokenRecipe])> {
    let at = tokens
        .iter()
        .position(|token| matches!(token.text.as_str(), "Fn" | "FnMut" | "FnOnce" | "fn"))?;
    if tokens.get(at + 1)?.text != "(" {
        return None;
    }
    let mut depth = 0i32;
    let mut close = None;
    for (offset, token) in tokens[at + 1..].iter().enumerate() {
        match token.text.as_str() {
            "(" | "[" | "<" => depth += 1,
            ")" | "]" | ">" => {
                depth -= 1;
                if depth == 0 {
                    close = Some(at + 1 + offset);
                    break;
                }
            }
            ">>" => depth -= 2,
            _ => {}
        }
    }
    let close = close?;
    let arguments: Vec<&[TokenRecipe]> = super::generics::split(&tokens[at + 2..close], ",")
        .into_iter()
        .filter(|argument| !argument.is_empty())
        .collect();
    let rest = &tokens[close + 1..];
    let result = match rest.first() {
        Some(arrow) if arrow.text == "->" => {
            let mut depth = 0i32;
            let mut end = rest.len();
            for (index, token) in rest.iter().enumerate().skip(1) {
                match token.text.as_str() {
                    "<" | "(" | "[" => depth += 1,
                    ">" | ")" | "]" => depth -= 1,
                    "+" | "," if depth == 0 => {
                        end = index;
                        break;
                    }
                    _ => {}
                }
                if depth < 0 {
                    end = index;
                    break;
                }
            }
            &rest[1..end]
        }
        _ => &rest[..0],
    };
    Some((arguments, result))
}

impl<'a> Component<'a> {
    /// A written Type of `tokens` in the scope of `signature`, or `None`
    /// when the shared Type service does not state it.
    pub(super) fn written_in(
        &mut self,
        db: &mut Database,
        signature: &FunctionSignature,
        tokens: &[TokenRecipe],
    ) -> QueryPoll<Option<TyTerm>> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        match crate::frontend::source_types::resolve_written_type(
            db,
            environment.names(),
            &sources,
            signature,
            tokens,
        ) {
            QueryPoll::Ready(Err(error)) if not_closed(&error) => QueryPoll::Ready(Ok(None)),
            other => other,
        }
    }

    /// The signature `tokens` state when they hold `Fn*(..) -> R` sugar or a
    /// `fn` pointer, resolved in the scope of `signature`.
    fn bound_from_tokens(
        &mut self,
        db: &mut Database,
        signature: &FunctionSignature,
        tokens: &[TokenRecipe],
    ) -> QueryPoll<Option<Arc<FnBound>>> {
        let Some((arguments, result)) = fn_sugar(tokens) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut params = Vec::with_capacity(arguments.len());
        for argument in arguments {
            params.push(ready!(self.written_in(db, signature, argument)));
        }
        let result = if result.is_empty() {
            Some(TyTerm::Primitive(PrimitiveType::Unit))
        } else {
            ready!(self.written_in(db, signature, result))
        };
        QueryPoll::Ready(Ok(Some(Arc::new(FnBound { params, result }))))
    }

    /// The `Fn*(..) -> R` bound of the callee's generic parameter `key`, from
    /// its own bound list and the `where` predicates naming it.
    pub(super) fn generic_fn_bound(
        &mut self,
        db: &mut Database,
        definition: Digest,
        key: &ParamKey,
    ) -> QueryPoll<Option<Arc<FnBound>>> {
        if let Some(found) = self.fn_bounds.get(&(definition, key.ordinal)) {
            return QueryPoll::Ready(Ok(found.clone()));
        }
        let found = ready!(self.generic_fn_bound_uncached(db, definition, key));
        self.fn_bounds
            .insert((definition, key.ordinal), found.clone());
        QueryPoll::Ready(Ok(found))
    }
    fn generic_fn_bound_uncached(
        &mut self,
        db: &mut Database,
        definition: Digest,
        key: &ParamKey,
    ) -> QueryPoll<Option<Arc<FnBound>>> {
        let Some(signature) = ready!(self.method_signature(db, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint callable signature differs")));
        };
        let Some(parameter) = signature.generics.iter().find(|parameter| {
            parameter.ordinal == key.ordinal && parameter.kind == GenericKind::Type
        }) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut candidates: Vec<&[TokenRecipe]> = vec![parameter.bounds.as_slice()];
        let predicates = match signature.predicates.split_first() {
            Some((first, rest)) if first.text == "where" => rest,
            _ => &signature.predicates[..],
        };
        for predicate in super::generics::split(predicates, ",") {
            if let [subject, colon, bounds @ ..] = predicate
                && subject.text == parameter.name
                && colon.text == ":"
            {
                candidates.push(bounds);
            }
        }
        for tokens in candidates {
            if let Some(bound) = ready!(self.bound_from_tokens(db, signature, tokens)) {
                return QueryPoll::Ready(Ok(Some(bound)));
            }
        }
        QueryPoll::Ready(Ok(None))
    }

    /// The `Fn*(..) -> R` signature of one of the executing function's own
    /// parameters that the path `node` reads: from the parameter's written
    /// Type (`impl Fn(A) -> R`, `&dyn Fn(A)`, `fn(A) -> R`), or the bounds of
    /// the generic parameter it is declared with.
    pub(super) fn parameter_fn_bound(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<Arc<FnBound>>> {
        let plan = self.plan;
        let Some((name, _)) = plan.name_of(node) else {
            return QueryPoll::Ready(Ok(None));
        };
        let definition = self.key.body.definition;
        let Some(signature) = ready!(self.method_signature(db, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint callable signature differs")));
        };
        let written = |parameter: &crate::syntax::signatures::ParameterRecipe| {
            if parameter.receiver {
                return false;
            }
            let tokens: Vec<&str> = parameter
                .pattern
                .iter()
                .map(|token| token.text.as_str())
                .collect();
            match tokens.as_slice() {
                [only] | ["mut", only] => only.strip_prefix("r#").unwrap_or(only) == name,
                _ => false,
            }
        };
        let mut matching = signature
            .parameters
            .iter()
            .filter(|parameter| written(parameter));
        let (Some(parameter), None) = (matching.next(), matching.next()) else {
            return QueryPoll::Ready(Ok(None));
        };
        if let Some(bound) = ready!(self.bound_from_tokens(db, signature, &parameter.ty)) {
            return QueryPoll::Ready(Ok(Some(bound)));
        }
        // A bare generic parameter, possibly behind references.
        let mut rest = parameter.ty.as_slice();
        while let [first, tail @ ..] = rest
            && (first.text == "&" || first.text == "mut" || first.text.starts_with('\''))
        {
            rest = tail;
        }
        if let [only] = rest
            && let Some(generic) = signature
                .generics
                .iter()
                .find(|generic| generic.kind == GenericKind::Type && generic.name == only.text)
        {
            let key = ParamKey {
                definition,
                ordinal: generic.ordinal,
            };
            return self.generic_fn_bound(db, definition, &key);
        }
        QueryPoll::Ready(Ok(None))
    }

    /// An argument of a call through a callable value: it takes the written
    /// parameter Type of the signature, deref-coerced as any argument is.
    pub(super) fn bound_argument(
        &mut self,
        db: &mut Database,
        call: NodeId,
        bound: &FnBound,
        ordinal: u32,
    ) -> QueryPoll<()> {
        let Some(Some(term)) = bound.params.get(ordinal as usize) else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        match ready!(self.coerce_argument(db, call, ordinal, term)) {
            Some(coerced) => {
                let current = self.via.clone();
                for via in coerced {
                    self.via = via;
                    ready!(self.constrain_term(db, call, term, ConstraintDemand::ParameterType));
                }
                self.via = current;
                QueryPoll::Ready(Ok(()))
            }
            None => self.constrain_term(db, call, term, ConstraintDemand::ParameterType),
        }
    }

    /// The value a call through a callable value produces.
    pub(super) fn bound_result(
        &mut self,
        db: &mut Database,
        call: NodeId,
        bound: &FnBound,
    ) -> QueryPoll<()> {
        match &bound.result {
            Some(term) => self.constrain_term(db, call, term, ConstraintDemand::ResultType),
            None => {
                self.defer(call, ConstraintDemand::ResultType);
                QueryPoll::Ready(Ok(()))
            }
        }
    }

    /// The `Fn*(..) -> R` signature of parameter `ordinal` of the selected
    /// function `definition`: the sugar of its written Type (`impl Fn(A) -> R`),
    /// or the bounds of the generic parameter it is declared with.
    fn parameter_callable_bound(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
        ordinal: u32,
    ) -> QueryPoll<Option<Arc<FnBound>>> {
        let Some(signature) = ready!(self.method_signature(db, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint callable signature differs")));
        };
        if let Some(parameter) = signature.parameters.get(ordinal as usize)
            && !parameter.receiver
            && let Some(bound) = ready!(self.bound_from_tokens(db, signature, &parameter.ty))
        {
            return QueryPoll::Ready(Ok(Some(bound)));
        }
        let Some(TyTerm::Param(key)) =
            ready!(self.quiet_parameter_term(db, call, definition, ordinal))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        if key.definition != definition {
            return QueryPoll::Ready(Ok(None));
        }
        self.generic_fn_bound(db, definition, &key)
    }

    /// A closure written as an argument of a plain call to a source function
    /// whose parameter of that position is a callable Type bounded by
    /// `Fn*(..) -> R`: the call, the callee and that bound. Reading the callee
    /// records no pending goal of this component.
    fn closure_argument_bound(
        &mut self,
        db: &mut Database,
        closure: NodeId,
    ) -> QueryPoll<Option<(NodeId, Digest, Arc<FnBound>)>> {
        let plan = self.plan;
        let saved = (self.pending, self.unrecorded);
        for edge in plan.uses(closure) {
            let Some(parent) = plan.fact(edge.parent) else {
                continue;
            };
            if parent.kind() != FactKind::Call || edge.ordinal == 0 {
                continue;
            }
            // A constructor call's arguments are its fields, not parameters.
            if ready!(self.constructor(db, ConstructorSite::Expression(edge.parent))).is_some() {
                continue;
            }
            let Some(Callee::Source(definition)) = ready!(self.callee(db, edge.parent)) else {
                continue;
            };
            let Some(bound) = ready!(self.parameter_callable_bound(
                db,
                edge.parent,
                definition,
                edge.ordinal - 1
            )) else {
                continue;
            };
            (self.pending, self.unrecorded) = saved;
            return QueryPoll::Ready(Ok(Some((edge.parent, definition, bound))));
        }
        (self.pending, self.unrecorded) = saved;
        QueryPoll::Ready(Ok(None))
    }

    /// The Type of a closure parameter pattern: its written Type, else the
    /// parameter Type of the callee the closure is passed to
    /// (`Fn*(A, ..)`'s `A`), instantiated by the call's other operands.
    pub(super) fn closure_parameter_type(
        &mut self,
        db: &mut Database,
        pattern: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        let plan = self.plan;
        let Some(fact) = plan.fact(pattern).filter(|fact| fact.closure_parameter()) else {
            return QueryPoll::Ready(Ok(None));
        };
        if fact.annotation_node().is_some() {
            return self.annotation_term(db, pattern);
        }
        for edge in plan.uses(pattern) {
            let Some(parent) = plan.fact(edge.parent) else {
                continue;
            };
            if parent.kind() != FactKind::Closure || edge.ordinal == 0 {
                continue;
            }
            let Some((call, definition, bound)) =
                ready!(self.closure_argument_bound(db, edge.parent))
            else {
                continue;
            };
            let Some(Some(term)) = bound.params.get(edge.ordinal as usize - 1) else {
                continue;
            };
            return QueryPoll::Ready(Ok(Some(ready!(
                self.instantiated_declared(db, call, definition, term)
            ))));
        }
        QueryPoll::Ready(Ok(None))
    }

    /// The current member is the value of the body of `closure`: when the
    /// closure is passed for a parameter bounded by `Fn*(..) -> R`, the body
    /// holds `R`.
    pub(super) fn closure_result_bound(
        &mut self,
        db: &mut Database,
        closure: NodeId,
    ) -> QueryPoll<()> {
        let Some((call, definition, bound)) = ready!(self.closure_argument_bound(db, closure))
        else {
            return QueryPoll::Ready(Ok(()));
        };
        match &bound.result {
            Some(term) => {
                self.constrain_declared(db, call, definition, term, ConstraintDemand::ResultType)
            }
            None => {
                self.defer(closure, ConstraintDemand::ResultType);
                QueryPoll::Ready(Ok(()))
            }
        }
    }

    /// The current member is a binding of the closure parameter pattern
    /// `pattern` without a written Type: when the closure is passed for a
    /// parameter bounded by `Fn*(A, ..) -> R`, the parameter holds the
    /// bound's `A`.
    pub(super) fn closure_parameter_bound(
        &mut self,
        db: &mut Database,
        pattern: NodeId,
    ) -> QueryPoll<()> {
        let plan = self.plan;
        for edge in plan.uses(pattern) {
            let Some(parent) = plan.fact(edge.parent) else {
                continue;
            };
            if parent.kind() != FactKind::Closure || edge.ordinal == 0 {
                continue;
            }
            let Some((call, definition, bound)) =
                ready!(self.closure_argument_bound(db, edge.parent))
            else {
                continue;
            };
            match bound.params.get(edge.ordinal as usize - 1) {
                Some(Some(term)) => ready!(self.constrain_declared(
                    db,
                    call,
                    definition,
                    term,
                    ConstraintDemand::ParameterType,
                )),
                _ => self.defer(pattern, ConstraintDemand::ParameterType),
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// The current member holds the callee's type parameter `key`
    /// (projected by `remaining`): every closure written for a parameter
    /// bounded by `Fn*(..) -> R` holds it too wherever the bound's Types name
    /// it, in its parameters and in its body value.
    pub(super) fn relate_fn_bounds(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
        key: &ParamKey,
        remaining: &[Step],
    ) -> QueryPoll<()> {
        let (operands, offset) = match self.fact(call) {
            Ok(fact) => (
                fact.operands().to_vec(),
                usize::from(!matches!(
                    fact.kind(),
                    FactKind::MethodCall | FactKind::LangCall
                )),
            ),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        for (position, argument) in operands.iter().enumerate().skip(offset) {
            let (Some(argument), Ok(ordinal)) = (argument, u32::try_from(position - offset)) else {
                continue;
            };
            let Some(closure) = ready!(self.closure_of(db, *argument, 0)) else {
                continue;
            };
            let Some(bound) = ready!(self.parameter_callable_bound(db, call, definition, ordinal))
            else {
                continue;
            };
            let closure_fact = match self.fact(closure) {
                Ok(fact) => fact,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let body = closure_fact.operand(0);
            for (index, term) in bound.params.iter().enumerate() {
                let (Some(term), Some(parameter)) = (term, closure_fact.operand(1 + index)) else {
                    continue;
                };
                for mut steps in parameter_paths(term, key) {
                    steps.extend_from_slice(remaining);
                    let current = std::mem::replace(
                        &mut self.via,
                        Via {
                            steps,
                            strict: false,
                        },
                    );
                    match ready!(self.pattern_binding(db, parameter)) {
                        Some(identity) => self.binding(identity, parameter),
                        // A destructuring closure parameter is not interpreted here.
                        None => self.mark_unrecorded(),
                    }
                    self.via = current;
                }
            }
            if let Some(result) = &bound.result {
                for mut steps in parameter_paths(result, key) {
                    steps.extend_from_slice(remaining);
                    self.expression_via(
                        body,
                        Via {
                            steps,
                            strict: false,
                        },
                    );
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }
}
