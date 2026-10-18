//! A numeric variable inside a generic parameter's Type, and that
//! parameter's trait bounds, in the source numeric constraint service.
//!
//! When the variable is the callee's type parameter itself, its bounds are
//! rustc's impl candidates for an integer or float variable
//! (`generics.rs::parameter_bounds`). When the variable is only a part of the
//! instantiated parameter (`T = Vec<?v>`, `T = &?v`), no impl of the bound
//! applies to the variable itself, and a bound relates the variable in one
//! of two ways:
//!
//! * a bound with no type arguments (`T: Clone`, `T: for<'a> Tr<'a>`) is an
//!   obligation on the whole Type and states nothing about its parts;
//! * an associated-type binding (`T: Iterator<Item = u8>`) equates the
//!   bound's projection of the instantiated parameter with the written Type,
//!   so a variable at some part of that projection is at the same part of
//!   the written Type. The projection is normalized by the shared
//!   associated-type normalizer, as a `for` head's `Item` is.
//!
//! A bound with trait type arguments (`T: Get<&'a isize>`) is selected by
//! rustc's impl selection over those arguments, which this service does not
//! repeat: the variable stays an unanswered goal.
use super::{Component, ConstraintDemand, RECURSION_LIMIT, Step, Via, iteration};
use crate::{
    core::digest::Digest,
    db::{memo::Database, poll::QueryPoll, value::QueryValue},
    syntax::{
        ast_adapter::TokenRecipe, signatures::GenericKind, source_constraints::FactKind,
    },
    traits::term::{ParamKey, TyTerm},
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

/// What one written bound says about a part of the bounded Type.
#[derive(Debug)]
enum BoundKind {
    /// No obligation on a part: a marker or plain trait, a lifetime, `?Sized`
    /// or `Fn*(..)` sugar (related through the closure written for it).
    Whole,
    /// Associated-type bindings `Name = Type`.
    Bindings(Vec<(String, Vec<TokenRecipe>)>),
    /// Trait type arguments, which rustc's impl selection decides.
    Arguments,
}

/// The index of the `>` that closes the `<` at `open`.
fn closing_angle(tokens: &[TokenRecipe], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.text.as_str() {
            "<" => depth += 1,
            ">" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            ">>" => {
                depth -= 2;
                if depth <= 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Classify one written bound (a `+`-separated item of a bound list).
fn classify_bound(mut tokens: &[TokenRecipe]) -> BoundKind {
    // `for<'a> Tr<'a>`: the binder introduces only lifetimes.
    if tokens.first().is_some_and(|token| token.text == "for")
        && tokens.get(1).is_some_and(|token| token.text == "<")
    {
        let Some(close) = closing_angle(tokens, 1) else {
            return BoundKind::Arguments;
        };
        tokens = &tokens[close + 1..];
    }
    let Some(first) = tokens.first() else {
        return BoundKind::Whole;
    };
    if first.text == "?" || first.text.starts_with('\'') {
        return BoundKind::Whole;
    }
    if super::callables::fn_sugar(tokens).is_some() {
        return BoundKind::Whole;
    }
    let Some(open) = tokens.iter().position(|token| token.text == "<") else {
        return BoundKind::Whole;
    };
    let Some(close) = closing_angle(tokens, open) else {
        return BoundKind::Arguments;
    };
    let mut bindings = Vec::new();
    for argument in super::generics::split(&tokens[open + 1..close], ",") {
        let Some(head) = argument.first() else {
            continue;
        };
        // A lifetime argument is a region, not a Type.
        if head.text.starts_with('\'') {
            continue;
        }
        match argument {
            [name, equals, value @ ..] if equals.text == "=" && !value.is_empty() => {
                bindings.push((name.text.clone(), value.to_vec()));
            }
            _ => return BoundKind::Arguments,
        }
    }
    if bindings.is_empty() {
        BoundKind::Whole
    } else {
        BoundKind::Bindings(bindings)
    }
}

impl<'a> Component<'a> {
    /// The written bounds of the callee's type parameter `key`: its own bound
    /// list and the `where` predicates naming it. `None` when a predicate
    /// mentions the parameter in a form not interpreted here (a projection or
    /// a reference to it as the bounded Type).
    fn written_bounds(
        &mut self,
        db: &mut Database,
        definition: Digest,
        key: &ParamKey,
    ) -> QueryPoll<Option<Vec<BoundKind>>> {
        let Some(signature) = ready!(self.method_signature(db, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(parameter) = signature
            .generics
            .iter()
            .find(|parameter| parameter.ordinal == key.ordinal && parameter.kind == GenericKind::Type)
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut lists: Vec<&[TokenRecipe]> = vec![parameter.bounds.as_slice()];
        let predicates = match signature.predicates.split_first() {
            Some((first, rest)) if first.text == "where" => rest,
            _ => &signature.predicates[..],
        };
        for predicate in super::generics::split(predicates, ",") {
            if predicate.is_empty() {
                continue;
            }
            let mentions = predicate.iter().any(|token| token.text == parameter.name);
            match predicate {
                [subject, colon, bounds @ ..]
                    if subject.text == parameter.name && colon.text == ":" =>
                {
                    lists.push(bounds);
                }
                // Another parameter's `Fn*(..)` bound names it through the
                // closure written for that parameter.
                [_, colon, bounds @ ..]
                    if colon.text == ":" && super::callables::fn_sugar(bounds).is_some() => {}
                _ if mentions => return QueryPoll::Ready(Ok(None)),
                _ => {}
            }
        }
        let mut bounds = Vec::new();
        for list in lists {
            for bound in super::generics::split(list, "+") {
                bounds.push(classify_bound(bound));
            }
        }
        QueryPoll::Ready(Ok(Some(bounds)))
    }

    /// The static Type of the callee's type parameter `key` at `call`: the
    /// part of the operand whose declared Type names it.
    fn parameter_actual(
        &mut self,
        db: &mut Database,
        call: crate::ir::pc::NodeId,
        definition: Digest,
        key: &ParamKey,
    ) -> QueryPoll<Option<TyTerm>> {
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
        for (position, operand) in operands.iter().enumerate().skip(offset) {
            let (Some(operand), Ok(ordinal)) = (operand, u32::try_from(position - offset)) else {
                continue;
            };
            let Some(declared) = ready!(self.quiet_parameter_term(db, call, definition, ordinal))
            else {
                continue;
            };
            let Some(path) = super::generics::parameter_paths(&declared, key)
                .into_iter()
                .next()
            else {
                continue;
            };
            let Some(actual) = ready!(self.expression_type(db, *operand)) else {
                continue;
            };
            if let Some(part) = ready!(self.project_term(db, &actual, &path)) {
                return QueryPoll::Ready(Ok(Some(part)));
            }
        }
        QueryPoll::Ready(Ok(None))
    }

    /// The current member is a part (`remaining`) of the instantiation of
    /// the callee's type parameter `key`, not the parameter itself: what the
    /// parameter's bounds say about that part.
    pub(super) fn inside_bounds(
        &mut self,
        db: &mut Database,
        call: crate::ir::pc::NodeId,
        definition: Digest,
        key: &ParamKey,
        remaining: &[Step],
    ) -> QueryPoll<()> {
        // A bound whose binding names another parameter with bindings of its
        // own is followed to rustc's recursion limit.
        if self.bound_depth >= RECURSION_LIMIT {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        }
        self.bound_depth += 1;
        let result = self.inside_bounds_inner(db, call, definition, key, remaining);
        self.bound_depth -= 1;
        result
    }
    fn inside_bounds_inner(
        &mut self,
        db: &mut Database,
        call: crate::ir::pc::NodeId,
        definition: Digest,
        key: &ParamKey,
        remaining: &[Step],
    ) -> QueryPoll<()> {
        let Some(bounds) = ready!(self.written_bounds(db, definition, key)) else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        for bound in bounds {
            match bound {
                BoundKind::Whole => {}
                BoundKind::Arguments => self.defer(call, ConstraintDemand::ParameterType),
                BoundKind::Bindings(bindings) => {
                    for (name, value) in bindings {
                        ready!(self.relate_binding(
                            db, call, definition, key, remaining, &name, &value
                        ));
                    }
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// `T: Trait<Name = value>`: the projection `<T as Trait>::Name` of the
    /// instantiated parameter is `value`, so the variable at `remaining`
    /// inside the parameter is at the same part of `value` wherever the
    /// projection holds that part of the parameter.
    fn relate_binding(
        &mut self,
        db: &mut Database,
        call: crate::ir::pc::NodeId,
        definition: Digest,
        key: &ParamKey,
        remaining: &[Step],
        name: &str,
        value: &[TokenRecipe],
    ) -> QueryPoll<()> {
        let Some(actual) = ready!(self.parameter_actual(db, call, definition, key)) else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(signature) = ready!(self.method_signature(db, definition)) else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(value) = ready!(self.written_in(db, signature, value)) else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        let positions: Vec<Vec<Step>> =
            match ready!(self.associated_answer_of(db, &actual, Digest::default(), name)) {
                iteration::ItemAnswer::Part(path) => remaining
                    .strip_prefix(path.as_slice())
                    .map(|rest| vec![rest.to_vec()])
                    .unwrap_or_default(),
                iteration::ItemAnswer::Structured { item, paths } => {
                    let mut holes = Vec::new();
                    iteration::item_holes(&item, &mut Vec::new(), &mut holes);
                    let mut positions = Vec::new();
                    for (hole, path) in paths.iter().enumerate() {
                        let Some(rest) = remaining.strip_prefix(path.as_slice()) else {
                            continue;
                        };
                        for (occurrence, within) in &holes {
                            if *occurrence as usize == hole {
                                let mut steps = within.clone();
                                steps.extend_from_slice(rest);
                                positions.push(steps);
                            }
                        }
                    }
                    positions
                }
                // The projection is closed: the variable is not inside it.
                iteration::ItemAnswer::Concrete(_) => Vec::new(),
                iteration::ItemAnswer::Unknown => {
                    self.defer(call, ConstraintDemand::ParameterType);
                    return QueryPoll::Ready(Ok(()));
                }
            };
        let current = self.via.clone();
        for steps in positions {
            self.via = Via {
                steps,
                strict: false,
            };
            ready!(self.constrain_declared(
                db,
                call,
                definition,
                &value,
                ConstraintDemand::ParameterType,
            ));
        }
        self.via = current;
        QueryPoll::Ready(Ok(()))
    }
}
