//! A callee's own type parameter in the source numeric constraint service.
//!
//! When a numeric variable instantiates a selected function's type
//! parameter `T` (an argument whose declared parameter Type is `T`, or a
//! call whose declared result is `T`), every other position of `T` in that
//! signature holds the same Type: the other arguments and the call's value
//! are related. `T`'s declared bounds are rustc obligations: a trait every
//! numeric type of the variable's family implements (the standard
//! formatting, comparison, clone and marker traits) leaves the variable to
//! its other relations and fallback; any other bound (which a unique impl
//! could select) stays a missing goal. Bounds are resolved through the
//! dispatcher's own written-path lookup from the callee's declaration
//! context, never by spelling.
use super::{Component, ConstraintDemand, Step, Via, fault};
use crate::{
    core::digest::Digest,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{op::PrimitiveType, pc::NodeId},
    source::manifest::Namespace,
    syntax::source_constraints::FactKind,
    syntax::{ast_adapter::TokenRecipe, signatures::GenericKind},
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

/// A declared Type term projected by a member's steps, stopping at a type
/// parameter.
pub(super) enum TermProjection {
    Leaf(TyTerm),
    Parameter(ParamKey, Vec<Step>),
    Unknown,
}
/// Projection paths from a declared Type term to type parameter `key`
/// (through references, tuples, arrays, slices and nominal type arguments).
pub(super) fn parameter_paths(term: &TyTerm, key: &ParamKey) -> Vec<Vec<Step>> {
    fn walk(term: &TyTerm, key: &ParamKey, prefix: &mut Vec<Step>, out: &mut Vec<Vec<Step>>) {
        match term {
            TyTerm::Param(found) if found == key => out.push(prefix.clone()),
            TyTerm::Ref(_, _, inner) => {
                prefix.push(Step::Deref);
                walk(inner, key, prefix, out);
                prefix.pop();
            }
            TyTerm::Tuple(members) => {
                for (position, member) in members.iter().enumerate() {
                    let Ok(position) = u32::try_from(position) else {
                        return;
                    };
                    prefix.push(Step::Field(position));
                    walk(member, key, prefix, out);
                    prefix.pop();
                }
            }
            TyTerm::Array(inner, _) | TyTerm::Slice(inner) => {
                prefix.push(Step::Element);
                walk(inner, key, prefix, out);
                prefix.pop();
            }
            // A nominal's own type arguments (`Box<T>`, `Option<T>`).
            TyTerm::Nominal(_, args) => {
                let types = args.iter().filter_map(|arg| match arg {
                    crate::traits::term::GenericArg::Type(inner) => Some(inner),
                    _ => None,
                });
                for (position, inner) in types.enumerate() {
                    let Ok(position) = u32::try_from(position) else {
                        return;
                    };
                    prefix.push(Step::TypeArgument(position));
                    walk(inner, key, prefix, out);
                    prefix.pop();
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(term, key, &mut Vec::new(), &mut out);
    out
}
/// The primitives an integer / float inference variable may become.
const INTEGERS: &[PrimitiveType] = &[
    PrimitiveType::I8,
    PrimitiveType::I16,
    PrimitiveType::I32,
    PrimitiveType::I64,
    PrimitiveType::I128,
    PrimitiveType::Isize,
    PrimitiveType::U8,
    PrimitiveType::U16,
    PrimitiveType::U32,
    PrimitiveType::U64,
    PrimitiveType::U128,
    PrimitiveType::Usize,
];
const FLOATS: &[PrimitiveType] = &[PrimitiveType::F32, PrimitiveType::F64];
/// Core traits whose primitive impls the facade does not declare as source
/// impls: rustc's builtin/auto marker candidates, and the formatting and
/// hashing impls the interpreter provides natively for every primitive.
/// Consulted only when the member index has no candidate for the trait
/// (held until the facade declares these impls).
const NATIVE_PRIMITIVE_TRAITS: &[(&str, &str)] = &[
    ("marker", "Sized"),
    ("marker", "Send"),
    ("marker", "Sync"),
    ("marker", "Unpin"),
    ("fmt", "Debug"),
    ("fmt", "Display"),
    ("fmt", "LowerHex"),
    ("fmt", "UpperHex"),
    ("fmt", "Octal"),
    ("fmt", "Binary"),
    ("fmt", "LowerExp"),
    ("fmt", "UpperExp"),
    ("hash", "Hash"),
    // The built-in operators of the numeric primitives (rustc's builtin
    // binary-op and unary-op impls): every integer type of a variable's
    // family implements them, so they leave the variable to its other
    // relations and the integer/float fallback.
    ("ops", "Add"),
    ("ops", "Sub"),
    ("ops", "Mul"),
    ("ops", "Div"),
    ("ops", "Rem"),
    ("ops", "Neg"),
    ("ops", "Not"),
    ("ops", "BitAnd"),
    ("ops", "BitOr"),
    ("ops", "BitXor"),
    ("ops", "Shl"),
    ("ops", "Shr"),
    ("ops", "AddAssign"),
    ("ops", "SubAssign"),
    ("ops", "MulAssign"),
    ("ops", "DivAssign"),
    ("ops", "RemAssign"),
    ("ops", "BitAndAssign"),
    ("ops", "BitOrAssign"),
    ("ops", "BitXorAssign"),
    ("ops", "ShlAssign"),
    ("ops", "ShrAssign"),
];
/// Whether `trait_key` is one of the core traits in
/// `NATIVE_PRIMITIVE_TRAITS`, by its facade declaration identity.
fn native_primitive_trait(
    db: &mut Database,
    sources: &[&crate::source::changes::PreparedSourceView],
    trait_key: Digest,
) -> bool {
    let Some(item) = sources
        .iter()
        .find_map(|source| source.outline_item(trait_key))
    else {
        return false;
    };
    let profile = match db.input(&InputKey::DefinitionHeader(trait_key)).as_deref() {
        Some(InputValue::DefinitionHeader(header)) => header.profile.clone(),
        _ => return false,
    };
    let key = &item.def_key;
    key.item_kind == crate::source::manifest::ItemKind::Trait
        && key.crate_key == crate::facade::unit(&profile, "core")
        && key.module_path.len() == 1
        && NATIVE_PRIMITIVE_TRAITS
            .iter()
            .any(|(module, name)| key.module_path[0] == *module && key.declared_name == *name)
}
/// What a type parameter's bounds say about the numeric variable it is.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BoundAnswer {
    /// Several impls apply (rustc: ambiguous, so the variable falls back).
    Unconstraining,
    /// Exactly one impl applies: rustc's selection unifies the variable.
    Selected(PrimitiveType),
    Unknown,
}
impl BoundAnswer {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Selected(a), Self::Selected(b)) if a != b => Self::Unknown,
            (Self::Selected(a), _) | (_, Self::Selected(a)) => Self::Selected(a),
            _ => Self::Unconstraining,
        }
    }
}
/// Split `tokens` at depth-0 occurrences of `separator`.
pub(super) fn split<'t>(tokens: &'t [TokenRecipe], separator: &str) -> Vec<&'t [TokenRecipe]> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (index, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "<" | "(" | "[" => depth += 1,
            ">" | ")" | "]" => depth -= 1,
            text if text == separator && depth == 0 => {
                parts.push(&tokens[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&tokens[start..]);
    parts
}
/// The supertrait paths of a trait declaration header (`trait A: B + C`,
/// and `where Self: D` predicates); `None` for a form not interpreted here.
fn supertrait_paths(header: &[TokenRecipe]) -> Option<Vec<String>> {
    let mut depth = 0i32;
    let mut colon = None;
    let mut clause = None;
    for (index, token) in header.iter().enumerate() {
        match token.text.as_str() {
            "<" | "(" | "[" => depth += 1,
            ">" | ")" | "]" => depth -= 1,
            ":" if depth == 0 && colon.is_none() && clause.is_none() => colon = Some(index),
            "where" if depth == 0 => {
                clause = Some(index);
                break;
            }
            "{" if depth == 0 => break,
            _ => {}
        }
    }
    let end = clause.unwrap_or(header.len());
    let end = header[..end]
        .iter()
        .position(|token| token.text == "{")
        .unwrap_or(end);
    let mut paths = match colon {
        Some(colon) if colon < end => bound_paths(&header[colon + 1..end])?,
        _ => Vec::new(),
    };
    if let Some(clause) = clause {
        let rest = &header[clause + 1..];
        let rest = &rest[..rest
            .iter()
            .position(|token| token.text == "{")
            .unwrap_or(rest.len())];
        for predicate in split(rest, ",") {
            match predicate {
                [] => {}
                [subject, colon, bounds @ ..] if subject.text == "Self" && colon.text == ":" => {
                    paths.extend(bound_paths(bounds)?)
                }
                _ if predicate.iter().any(|token| token.text == "Self") => return None,
                _ => {}
            }
        }
    }
    Some(paths)
}
/// The written trait paths of one bound list; `None` for a bound form whose
/// obligation this service does not interpret (`Fn(..)` sugar, `for<..>`).
fn bound_paths(tokens: &[TokenRecipe]) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    for bound in split(tokens, "+") {
        let Some(first) = bound.first() else {
            continue;
        };
        // `?Sized` relaxes; a lifetime bound is not a trait obligation.
        if first.text == "?" || first.text.starts_with('\'') {
            continue;
        }
        let mut path = String::new();
        for token in bound {
            match token.text.as_str() {
                "<" => break,
                "(" | "for" | "dyn" | "impl" | "~" | "const" => return None,
                text => path.push_str(text),
            }
        }
        if path.is_empty() {
            return None;
        }
        paths.push(path);
    }
    Some(paths)
}

impl<'a> Component<'a> {
    /// A declared Type term projected by `steps`, stopping at a type
    /// parameter (nominal field steps go through the shared schema binder).
    pub(super) fn project_parametric(
        &mut self,
        db: &mut Database,
        term: &TyTerm,
        steps: &[Step],
    ) -> QueryPoll<TermProjection> {
        self.note_depth(super::term_depth(term));
        let mut term = term.clone();
        for (index, step) in steps.iter().enumerate() {
            // Indexing and field access auto-deref through references and
            // boxes before their step: a parameter reached there (`Box<T>`
            // indexed as `b[i]`) holds the rest of the projection.
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
            if let TyTerm::Param(key) = term {
                return QueryPoll::Ready(Ok(TermProjection::Parameter(
                    key,
                    steps[index..].to_vec(),
                )));
            }
            // `*b` of a `Box<T>`: its contents `T` (possibly a parameter).
            if *step == Step::Deref
                && let Some(contents) = ready!(self.box_contents(db, &term))
            {
                term = contents;
                continue;
            }
            match ready!(self.project_term(db, &term, std::slice::from_ref(step))) {
                Some(next) => term = next,
                None => return QueryPoll::Ready(Ok(TermProjection::Unknown)),
            }
        }
        QueryPoll::Ready(Ok(match term {
            TyTerm::Param(key) => TermProjection::Parameter(key, Vec::new()),
            term => TermProjection::Leaf(term),
        }))
    }
    /// The declared goals of a selected function, read without recording
    /// this component's own pending demands.
    pub(super) fn quiet_parameter_term(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
        ordinal: u32,
    ) -> QueryPoll<Option<TyTerm>> {
        let saved = (self.pending, self.unrecorded);
        let term = self.parameter_term(db, node, definition, ordinal);
        (self.pending, self.unrecorded) = saved;
        term
    }
    pub(super) fn quiet_result_term(
        &mut self,
        db: &mut Database,
        node: NodeId,
        definition: Digest,
    ) -> QueryPoll<Option<TyTerm>> {
        let saved = (self.pending, self.unrecorded);
        let term = self.result_term(db, node, definition);
        (self.pending, self.unrecorded) = saved;
        term
    }
    /// The current member (an argument of `call`, or `call` itself) holds
    /// the callee's type parameter `key`, projected by `remaining`.
    pub(super) fn relate_parameter(
        &mut self,
        db: &mut Database,
        call: NodeId,
        definition: Digest,
        key: ParamKey,
        remaining: Vec<Step>,
    ) -> QueryPoll<()> {
        if key.definition != definition {
            // An impl's or trait's parameter is bound by the receiver/owner.
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        }
        // Explicit generic arguments (`f::<u8>(..)`) fix the parameter: the
        // shared indexed explicit-Type service answers it.
        if self
            .fact(call)
            .is_ok_and(|fact| fact.kind() == FactKind::Call && fact.flag())
        {
            return self.explicit_parameter(db, call, &key, remaining);
        }
        let (arguments, offset) = match self.fact(call) {
            // A method call's operands are its receiver and arguments, in
            // parameter order, as a language-item call's are its arguments;
            // a call's first operand is its callee.
            Ok(fact) => (
                fact.operands().to_vec(),
                usize::from(!matches!(
                    fact.kind(),
                    FactKind::MethodCall | FactKind::LangCall
                )),
            ),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let strict = self.via.strict;
        let method = self
            .fact(call)
            .is_ok_and(|fact| fact.kind() == FactKind::MethodCall);
        let related = |component: &mut Self, node: NodeId, path: Vec<Step>| {
            let scalar = path.is_empty();
            let mut steps = path;
            steps.extend_from_slice(&remaining);
            component.expression_via(
                Some(node),
                Via {
                    steps,
                    strict: strict && scalar,
                },
            );
        };
        for (position, argument) in arguments.iter().enumerate().skip(offset) {
            let (Some(argument), Ok(ordinal)) = (argument, u32::try_from(position - offset)) else {
                continue;
            };
            if let Some(term) = ready!(self.quiet_parameter_term(db, call, definition, ordinal)) {
                // A method receiver expression is adjusted (auto-ref and
                // auto-deref) to the declared `self` Type: the part sits
                // behind exactly the references of the receiver's own static
                // Type (the probe strips references, never a box), which the
                // declared Type's leading `&` only restates. A receiver whose
                // Type the services do not state keeps `AutoDeref`.
                let references = if method && position == 0 {
                    match ready!(self.expression_type(db, *argument)) {
                        Some(mut receiver) => {
                            let mut references = 0usize;
                            while let TyTerm::Ref(_, _, inner) = receiver {
                                references += 1;
                                receiver = *inner;
                            }
                            Some(references)
                        }
                        None => None,
                    }
                } else {
                    None
                };
                for path in parameter_paths(&term, &key) {
                    let path = if method && position == 0 {
                        let rest = path
                            .iter()
                            .position(|step| *step != Step::Deref)
                            .unwrap_or(path.len());
                        let mut adjusted = Vec::with_capacity(path.len() - rest + 1);
                        match references {
                            Some(references) => {
                                adjusted.extend(std::iter::repeat_n(Step::Deref, references))
                            }
                            // The receiver is the parameter itself, auto-ref'd.
                            None if rest == path.len() => {}
                            None => adjusted.push(Step::AutoDeref),
                        }
                        adjusted.extend_from_slice(&path[rest..]);
                        adjusted
                    } else {
                        path
                    };
                    related(self, *argument, path);
                }
            }
        }
        if let Some(term) = ready!(self.quiet_result_term(db, call, definition)) {
            for path in parameter_paths(&term, &key) {
                related(self, call, path);
            }
        }
        // Closures written for parameters bounded by `Fn*(..) -> R` hold the
        // parameter wherever that bound names it.
        ready!(self.relate_fn_bounds(db, call, definition, &key, &remaining));
        // The variable is only a part of the instantiated parameter: the
        // parameter's bounds are obligations on the whole Type.
        if !remaining.is_empty() {
            return self.inside_bounds(db, call, definition, &key, &remaining);
        }
        match ready!(self.parameter_bounds(db, definition, &key)) {
            BoundAnswer::Unconstraining => {}
            // The variable is the parameter itself: the one applicable impl
            // fixes it.
            BoundAnswer::Selected(primitive) => {
                if let Err(error) = self.constrain(call, primitive) {
                    return QueryPoll::Ready(Err(error));
                }
            }
            BoundAnswer::Unknown => self.defer(call, ConstraintDemand::ParameterType),
        }
        QueryPoll::Ready(Ok(()))
    }
    /// A parameter written explicitly at the call: its selected Type
    /// projected by the remaining steps.
    fn explicit_parameter(
        &mut self,
        db: &mut Database,
        call: NodeId,
        key: &ParamKey,
        remaining: Vec<Step>,
    ) -> QueryPoll<()> {
        let explicit = match db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedExplicitTypeArguments(super::entry(self.key, call)),
        ) {
            QueryPoll::Ready(Ok(explicit)) => explicit,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                self.defer(call, ConstraintDemand::ParameterType);
                return QueryPoll::Ready(Ok(()));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::PreparedExplicitTypeArguments(explicit) = explicit.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint explicit Type query differs")));
        };
        let Some(term) = explicit
            .rows()
            .iter()
            .find(|row| row.ordinal() == key.ordinal)
            .map(|row| row.source().source().clone())
        else {
            self.defer(call, ConstraintDemand::ParameterType);
            return QueryPoll::Ready(Ok(()));
        };
        let strict = self.via.strict;
        let current = std::mem::replace(
            &mut self.via,
            Via {
                steps: remaining,
                strict,
            },
        );
        let result = self.constrain_term(db, call, &term, ConstraintDemand::ParameterType);
        self.via = current;
        result
    }
    /// The written trait paths bounding type parameter `key` of function
    /// `definition`: its own bounds and the `where` predicates naming it.
    /// `None` when some bound or predicate mentioning it is a form not
    /// interpreted here.
    fn declared_bound_paths(
        &mut self,
        db: &mut Database,
        definition: Digest,
        key: &ParamKey,
    ) -> QueryPoll<Option<Vec<String>>> {
        let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint generic signature differs")));
        };
        let Some(parameter) = signature.generics.iter().find(|parameter| {
            parameter.ordinal == key.ordinal && parameter.kind == GenericKind::Type
        }) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(mut paths) = bound_paths(&parameter.bounds) else {
            return QueryPoll::Ready(Ok(None));
        };
        // `where` predicates naming the parameter add bounds; any other
        // predicate mentioning it is not interpreted here.
        let predicates = match signature.predicates.split_first() {
            Some((first, rest)) if first.text == "where" => rest,
            _ => &signature.predicates[..],
        };
        for predicate in split(predicates, ",") {
            if predicate.is_empty() {
                continue;
            }
            let mentions = predicate.iter().any(|token| token.text == parameter.name);
            match predicate {
                [subject, colon, bounds @ ..]
                    if subject.text == parameter.name && colon.text == ":" =>
                {
                    match bound_paths(bounds) {
                        Some(found) => paths.extend(found),
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                // Another parameter's `Fn*(..) -> R` bound names the parameter
                // only through the closure written for it
                // (`relate_fn_bounds`): no obligation on this parameter.
                [_, colon, bounds @ ..]
                    if colon.text == ":" && super::callables::fn_sugar(bounds).is_some() => {}
                _ if mentions => return QueryPoll::Ready(Ok(None)),
                _ => {}
            }
        }
        QueryPoll::Ready(Ok(Some(paths)))
    }
    /// A method called on a receiver of the executing function's own type
    /// parameter `key`: the declaration of `name` in the one trait of its
    /// elaborated bounds (written bounds and their supertraits) that declares
    /// it, as rustc's param-env candidates select it. Impl/trait parameters
    /// are not consulted here.
    pub(super) fn bound_method(
        &mut self,
        db: &mut Database,
        key: &ParamKey,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let definition = self.key.body.definition;
        if key.definition != definition {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(paths) = ready!(self.declared_bound_paths(db, definition, key)) else {
            return QueryPoll::Ready(Ok(None));
        };
        self.elaborated_method(
            db,
            Vec::new(),
            paths.into_iter().map(|path| (definition, path)).collect(),
            name,
        )
    }
    /// A method called on a trait object `dyn Principal` (through any
    /// references and boxes): the declaration of `name` in the principal
    /// trait or its supertraits, as rustc's object candidates select it.
    pub(super) fn dyn_method(
        &mut self,
        db: &mut Database,
        principal: Digest,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        self.elaborated_method(db, vec![principal], Vec::new(), name)
    }
    /// The one declaration of `name` among `roots` (trait identities), the
    /// traits the written `pending` paths name (each resolved from its
    /// scope's definition), and all their supertraits (resolved in the
    /// subtrait's own module).
    fn elaborated_method(
        &mut self,
        db: &mut Database,
        mut roots: Vec<Digest>,
        mut pending: Vec<(Digest, String)>,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let resolver =
            match crate::frontend::callables::CallResolver::new(environment.names(), &sources) {
                Ok(resolver) => resolver,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        // The elaborated param-env: the bounds, then each bound trait's
        // supertraits (resolved in that trait's own module). Each trait is
        // visited once, so the walk is finite.
        let mut traits: Vec<Digest> = Vec::new();
        let mut declared = Vec::new();
        loop {
            let identity = if let Some(identity) = roots.pop() {
                identity
            } else if let Some((scope, path)) = pending.pop() {
                let found = match resolver.lookup_written(db, scope, &path, Namespace::Type) {
                    QueryPoll::Ready(Ok(found)) => found,
                    QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                        return QueryPoll::Ready(Ok(None));
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let crate::frontend::names::NameLookup::Defined(bound) = found else {
                    return QueryPoll::Ready(Ok(None));
                };
                bound.identity
            } else {
                break;
            };
            if traits.contains(&identity) {
                continue;
            }
            traits.push(identity);
            if let Some(function) = members.trait_declaration(identity, name)
                && !declared.contains(&function)
            {
                declared.push(function);
            }
            let Some(item) = sources
                .iter()
                .find_map(|source| source.outline_item(identity))
            else {
                return QueryPoll::Ready(Ok(None));
            };
            let Some(supertraits) = supertrait_paths(&item.header.tokens) else {
                return QueryPoll::Ready(Ok(None));
            };
            pending.extend(supertraits.into_iter().map(|path| (identity, path)));
        }
        QueryPoll::Ready(Ok(match declared.as_slice() {
            [function] => Some(*function),
            _ => None,
        }))
    }
    /// The traits among `traits` that declare `name` as a method: an
    /// associated function without a receiver is no method candidate.
    fn method_traits(
        &mut self,
        db: &mut Database,
        members: &crate::frontend::member_index::MemberIndex,
        traits: Vec<Digest>,
        name: &str,
    ) -> QueryPoll<Vec<Digest>> {
        let mut kept = Vec::with_capacity(traits.len());
        for candidate in traits {
            let Some(function) = members.trait_declaration(candidate, name) else {
                kept.push(candidate);
                continue;
            };
            let Some(signature) = ready!(self.method_signature(db, function)) else {
                kept.push(candidate);
                continue;
            };
            let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
                return QueryPoll::Ready(Err(fault("constraint method signature differs")));
            };
            if signature
                .parameters
                .first()
                .is_some_and(|parameter| parameter.receiver)
            {
                kept.push(candidate);
            }
        }
        QueryPoll::Ready(Ok(kept))
    }
    /// The declaration of the method `name` called on the unsuffixed literal
    /// `receiver` (or a local holding one): rustc probes the traits in scope
    /// that the literal's family implements; exactly one providing `name` as
    /// a method (and no inherent method of a primitive) selects its
    /// declaration, whose declared signature then relates the call.
    pub(super) fn numeric_receiver_method(
        &mut self,
        db: &mut Database,
        receiver: NodeId,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let Some((_, floating)) = ready!(self.literal_origin(db, receiver)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let family = if floating { FLOATS } else { INTEGERS };
        let (false, traits) = members.primitive_method_traits(family, name) else {
            return QueryPoll::Ready(Ok(None));
        };
        let traits = ready!(self.method_traits(db, &members, traits, name));
        QueryPoll::Ready(Ok(match traits.as_slice() {
            [only] => members.trait_declaration(*only, name),
            _ => None,
        }))
    }
    /// The numeric variable coerced to a trait object `dyn Principal` (an
    /// unsizing coercion): rustc's obligation `?v: Principal`. One applicable
    /// impl selects the variable's Type, several leave it to fallback.
    pub(super) fn object_obligation(
        &mut self,
        db: &mut Database,
        node: NodeId,
        principal: Digest,
    ) -> QueryPoll<()> {
        let Some(environment) = ready!(self.environment(db)) else {
            self.defer(node, ConstraintDemand::AnnotationType);
            return QueryPoll::Ready(Ok(()));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        match ready!(self.trait_bound(db, &members, &sources, principal, 0)) {
            BoundAnswer::Unconstraining => QueryPoll::Ready(Ok(())),
            BoundAnswer::Selected(primitive) => QueryPoll::Ready(self.constrain(node, primitive)),
            BoundAnswer::Unknown => {
                self.defer(node, ConstraintDemand::AnnotationType);
                QueryPoll::Ready(Ok(()))
            }
        }
    }
    /// A method called on the numeric variable itself (`4.f()`): rustc
    /// probes the traits in scope providing `name`; with exactly one such
    /// trait, the variable takes that trait's impl candidates (one impl fixes
    /// it, several leave it to fallback). An inherent method of a primitive,
    /// or several traits, needs the receiver's Type from elsewhere.
    pub(super) fn numeric_receiver(&mut self, db: &mut Database, call: NodeId) -> QueryPoll<()> {
        let name = match self.fact(call) {
            Ok(fact) => fact.member().and_then(|row| self.plan.member(row)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(name) = name.map(str::to_owned) else {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let family = if self.floating { FLOATS } else { INTEGERS };
        let answer = match members.primitive_method_traits(family, &name) {
            (false, traits) => {
                let traits = ready!(self.method_traits(db, &members, traits, &name));
                match traits.as_slice() {
                    [only] => ready!(self.trait_bound(db, &members, &sources, *only, 0)),
                    _ => BoundAnswer::Unknown,
                }
            }
            (true, _) => BoundAnswer::Unknown,
        };
        match answer {
            BoundAnswer::Unconstraining => QueryPoll::Ready(Ok(())),
            BoundAnswer::Selected(primitive) => QueryPoll::Ready(self.constrain(call, primitive)),
            BoundAnswer::Unknown => {
                self.defer(call, ConstraintDemand::MethodSelection);
                QueryPoll::Ready(Ok(()))
            }
        }
    }
    /// What the declared bounds of `key` (its own and its `where`
    /// predicates) say about a numeric variable of this component's family
    /// that instantiates it: rustc's impl candidates for `?v: Bound`, from
    /// the run's impl member index.
    fn parameter_bounds(
        &mut self,
        db: &mut Database,
        definition: Digest,
        key: &ParamKey,
    ) -> QueryPoll<BoundAnswer> {
        let Some(paths) = ready!(self.declared_bound_paths(db, definition, key)) else {
            return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
        };
        if paths.is_empty() {
            return QueryPoll::Ready(Ok(BoundAnswer::Unconstraining));
        }
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let resolver =
            match crate::frontend::callables::CallResolver::new(environment.names(), &sources) {
                Ok(resolver) => resolver,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let mut answer = BoundAnswer::Unconstraining;
        for path in paths {
            let found = match resolver.lookup_written(db, definition, &path, Namespace::Type) {
                QueryPoll::Ready(Ok(found)) => found,
                QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                    return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let crate::frontend::names::NameLookup::Defined(trait_definition) = found else {
                return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
            };
            let bound =
                ready!(self.trait_bound(db, &members, &sources, trait_definition.identity, 0));
            answer = answer.and(bound);
        }
        QueryPoll::Ready(Ok(answer))
    }
    /// rustc's candidates for `?v: Trait`: several applicable impls leave
    /// the variable ambiguous (it falls back), one fixes it, a lone blanket
    /// impl passes the obligation on to its own bounds (to rustc's
    /// `recursion_limit` depth).
    fn trait_bound(
        &mut self,
        db: &mut Database,
        members: &crate::frontend::member_index::MemberIndex,
        sources: &[&crate::source::changes::PreparedSourceView],
        trait_key: Digest,
        depth: u32,
    ) -> QueryPoll<BoundAnswer> {
        if depth > 128 {
            return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
        }
        let family = if self.floating { FLOATS } else { INTEGERS };
        let candidates = members.primitive_candidates(trait_key, family);
        if candidates.unresolved {
            return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
        }
        QueryPoll::Ready(Ok(
            match (candidates.direct.as_slice(), candidates.blankets.as_slice()) {
                ([only], []) => BoundAnswer::Selected(*only),
                ([], [bounds]) => {
                    let mut answer = BoundAnswer::Unconstraining;
                    for bound in bounds {
                        let Some(bound) = bound else {
                            return QueryPoll::Ready(Ok(BoundAnswer::Unknown));
                        };
                        let nested =
                            ready!(self.trait_bound(db, members, sources, *bound, depth + 1));
                        answer = answer.and(nested);
                    }
                    answer
                }
                ([], []) => {
                    if native_primitive_trait(db, sources, trait_key) {
                        BoundAnswer::Unconstraining
                    } else {
                        BoundAnswer::Unknown
                    }
                }
                _ => BoundAnswer::Unconstraining,
            },
        ))
    }
}
