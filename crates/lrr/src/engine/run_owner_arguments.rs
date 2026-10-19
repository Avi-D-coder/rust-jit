//! Type arguments written on a call path's owner segment: `Vec::<T>::new()`,
//! `W::<u8>::f()`, `Box::<[(); 0]>::new(..)`. They are the Type arguments of
//! the owner type, `<W<u8>>::f()`, and bind the parameters of the impl that
//! provides the selected associated function.
//!
//! Admitted bodies retain final/owner Type occurrences in SourceConstraints.
//! The shared explicit path argument selector uses the same SourceLocalPlan and
//! entered Type service as constructor constraints and record preparation. Each
//! source PC/position keeps immutable arguments and guards; descriptor bindings
//! stay local to an activation. Bodies without an admitted environment retain
//! the older signature-context route as held migration debt below.
//! The bindings join the call's explicit Type arguments
//! (`SelectedArgumentCall::explicit`), the route `f::<u8>()` binds the
//! function's own parameters through; the callee, its argument hints and its
//! contracts are the ordinary call's.
use super::*;
use crate::{
    db::dependency::{InputKey, InputValue},
    frontend::explicit_type_arguments::ExplicitType,
    ir::pc::Phase,
    traits::term::{GenericArg, TyTerm},
};
use ra_ap_syntax::{AstNode, ast, ast::HasGenericArgs};

/// The Type arguments written on a call path's owner segment.
pub(super) struct OwnerArguments {
    /// The written Type arguments in order; `None` for the hole `_`.
    arguments: Vec<Option<OwnerArgument>>,
    /// The certified inputs the selections depend on.
    guards: Vec<Arc<crate::db::memo::PreparedInputGuard>>,
}
pub(super) use crate::frontend::explicit_type_arguments::PathArgumentPosition as ArgumentPosition;
/// One written owner Type argument.
pub(super) enum OwnerArgument {
    /// Selected at its indexed Type occurrence of an original-source body.
    Entered(ExplicitType),
    /// Resolved in the declaring signature's context (a generated body).
    Term(TyTerm),
}
impl OwnerArguments {
    /// Exact Type arguments under the current activation. The immutable
    /// selection is shared; descriptors stay local to this execution.
    pub(super) fn value_descriptors(
        &self,
        frame: &Frame,
        store: &mut GuestStore,
    ) -> Result<Vec<Option<crate::runtime::shape::TypeId>>, Diagnostic> {
        self.arguments
            .iter()
            .map(|argument| {
                argument
                    .as_ref()
                    .map(|argument| argument.descriptor(frame, store))
                    .transpose()
            })
            .collect()
    }
}
impl OwnerArgument {
    fn source(&self) -> &TyTerm {
        match self {
            Self::Entered(argument) => argument.source(),
            Self::Term(term) => term,
        }
    }
    /// The run-local descriptor under the calling frame.
    fn descriptor(
        &self,
        frame: &Frame,
        store: &mut GuestStore,
    ) -> Result<crate::runtime::shape::TypeId, Diagnostic> {
        match self {
            Self::Entered(argument) => argument.descriptor(frame, store),
            Self::Term(TyTerm::Param(key)) => frame.parameter_hint(*key)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "owner caller parameter has no reached runtime binding".into(),
                )
            }),
            Self::Term(term) => store.intern_structural_term(term)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "owner Type argument of a generated body names a Type with no structural descriptor"
                        .into(),
                )
            }),
        }
    }
}

/// The impl parameters (callee generic ordinals) a pattern Type fixes when
/// matched against the written owner Type. A parameter of the callee binds
/// to the corresponding part of the written Type.
fn match_owner(
    pattern: &TyTerm,
    actual: &TyTerm,
    callee: crate::core::digest::Digest,
    bound: &mut Vec<(u32, TyTerm)>,
) {
    match (pattern, actual) {
        (TyTerm::Param(key), _) if key.definition == callee => {
            if !bound.iter().any(|(ordinal, _)| *ordinal == key.ordinal) {
                bound.push((key.ordinal, actual.clone()));
            }
        }
        (TyTerm::Nominal(left, patterns), TyTerm::Nominal(right, actuals)) if left == right => {
            let types = |arguments: &'_ [GenericArg]| -> Vec<TyTerm> {
                arguments
                    .iter()
                    .filter_map(|argument| match argument {
                        GenericArg::Type(ty) => Some(ty.clone()),
                        _ => None,
                    })
                    .collect()
            };
            for (pattern, actual) in types(patterns).iter().zip(types(actuals).iter()) {
                match_owner(pattern, actual, callee, bound);
            }
        }
        (TyTerm::Ref(_, _, left), TyTerm::Ref(_, _, right))
        | (TyTerm::Slice(left), TyTerm::Slice(right))
        | (TyTerm::Array(left, _), TyTerm::Array(right, _)) => {
            match_owner(left, right, callee, bound)
        }
        (TyTerm::Tuple(left), TyTerm::Tuple(right)) => {
            for (pattern, actual) in left.iter().zip(right) {
                match_owner(pattern, actual, callee, bound);
            }
        }
        _ => {}
    }
}

impl RunMachine {
    /// The owner-segment Type arguments of the call at the frame's PC, or
    /// `None` when its path writes none. Selected once per call site; later
    /// uses check the selection's certified inputs.
    pub(super) fn owner_arguments(
        &mut self,
        db: &mut Database,
    ) -> QueryPoll<Option<Arc<OwnerArguments>>> {
        self.path_arguments(db, ArgumentPosition::Owner)
    }
    pub(super) fn constructor_arguments(
        &mut self,
        db: &mut Database,
    ) -> QueryPoll<Option<Arc<OwnerArguments>>> {
        self.path_arguments(db, ArgumentPosition::Constructor)
    }
    fn path_arguments(
        &mut self,
        db: &mut Database,
        position: ArgumentPosition,
    ) -> QueryPoll<Option<Arc<OwnerArguments>>> {
        macro_rules! ready {
            ($e:expr) => {
                match $e {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let key = ready!(
            self.frame
                .body
                .pc(NodeId(self.frame.pc().node), Phase::Entry)
        );
        let cache_key = (key.clone(), position);
        if let Some(cached) = self.owner_arguments.get(&cache_key) {
            return QueryPoll::Ready(match cached {
                None => Ok(None),
                Some(found) => found
                    .guards
                    .iter()
                    .try_for_each(|guard| guard.require_current(db))
                    .map(|()| Some(Arc::clone(found))),
            });
        }
        // Admitted source uses the retained syntax plan shared with source
        // constraints and records. No AST/list walk is repeated at call phases.
        if db.input(&InputKey::BodyEnvironment(key.body.clone())).is_some() {
            let plan_key = QueryKey::SourceLocalPlan(key.body.clone());
            let value = poll!(db.fetch(&SyntaxProvider, plan_key.clone()));
            let QueryValue::SourceLocalPlan(plan) = value.value.as_ref() else {
                return QueryPoll::Ready(Err(call_fault("path argument source plan differs")));
            };
            let selected = poll!(crate::frontend::explicit_type_arguments::select_path_arguments(
                db, plan, &key, position));
            let found = match selected {
                None => None,
                Some(selected) => {
                    let guard = poll!(db.prepare_input_guard(&SyntaxProvider, &plan_key, value.fingerprint));
                    let mut guards = selected.guards;
                    guards.push(Arc::new(guard));
                    Some(Arc::new(OwnerArguments {
                        arguments: selected.arguments.into_iter()
                            .map(|argument| argument.map(OwnerArgument::Entered)).collect(),
                        guards,
                    }))
                }
            };
            self.owner_arguments.insert(cache_key, found.clone());
            return QueryPoll::Ready(Ok(found));
        }
        // The written Type arguments of the callee path's owner segment, from
        // the executing body's cached parse: no source plan is prepared for a
        // call that writes none.
        let definition = key.body.definition;
        // A body published without its source and body inputs writes no
        // source path.
        let source = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
            Some(InputValue::DefinitionSource(source)) => source.clone(),
            _ => return QueryPoll::Ready(Ok(None)),
        };
        let handle = match db.input(&InputKey::DefinitionBody(definition)).as_deref() {
            Some(InputValue::DefinitionBody(handle)) if handle.key() == key.body => handle.clone(),
            _ => return QueryPoll::Ready(Ok(None)),
        };
        let parsed = poll!(db.fetch(&SyntaxProvider, QueryKey::ParseFile(source)));
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("reached call parse kind differs")));
        };
        let syntax =
            ready!(parsed.entered_node_syntax(&handle, &self.frame.body, NodeId(key.node)));
        let value_path = syntax
            .as_ref()
            .and_then(|node| ast::PathExpr::cast(node.clone()));
        // A constructor value states arguments on its final segment, just as
        // an associated call states them on its owner segment. Both consume
        // the same indexed Type-argument service below.
        let path = match value_path.as_ref() {
            Some(path) => path.path(),
            None => syntax
                .and_then(ast::CallExpr::cast)
                .and_then(|call| match call.expr() {
                    Some(ast::Expr::PathExpr(path)) => path.path(),
                    _ => None,
                }),
        };
        let list = path.and_then(|path| {
            let final_arguments = || path.segment().and_then(|s| s.generic_arg_list());
            let owner_arguments = || path.qualifier().and_then(|p| p.segment())
                .and_then(|s| s.generic_arg_list());
            match position {
                ArgumentPosition::Constructor => final_arguments().or_else(owner_arguments),
                ArgumentPosition::Owner if value_path.is_some() => final_arguments(),
                ArgumentPosition::Owner => owner_arguments(),
            }
        });
        let Some(list) = list else {
            self.owner_arguments.insert(cache_key, None);
            return QueryPoll::Ready(Ok(None));
        };
        let mut types = Vec::new();
        for argument in list.generic_args() {
            match argument {
                ast::GenericArg::TypeArg(argument) => match argument.ty() {
                    Some(ty) => types.push(ty),
                    None => return QueryPoll::Ready(Err(call_fault("owner Type argument absent"))),
                },
                ast::GenericArg::LifetimeArg(_) => {}
                _ => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "owner const or associated generic argument requires its typed source goal"
                            .into(),
                    )));
                }
            }
        }
        let found = if value_path.is_none() && position == ArgumentPosition::Owner {
            poll!(self.signature_owner_arguments(db, parsed, &key, &types))
        } else {
            return QueryPoll::Ready(Err(call_fault(
                "explicit value Type arguments require their admitted indexed body environment",
            )));
        };
        let found = Arc::new(found);
        self.owner_arguments.insert(cache_key, Some(Arc::clone(&found)));
        QueryPoll::Ready(Ok(Some(found)))
    }

    /// The written owner arguments of a macro-generated body, which has no
    /// published body environment: resolved in its declaring signature's
    /// context, as the declaration's own generic bounds and parameter types
    /// are (a generated `$crate` root names its defining crate).
    fn signature_owner_arguments(
        &mut self,
        db: &mut Database,
        parsed: &crate::syntax::parse::ParsedFile,
        key: &PcKey,
        types: &[ast::Type],
    ) -> QueryPoll<OwnerArguments> {
        let signature = match db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(key.body.definition),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("caller signature query variant mismatch")));
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut arguments = Vec::new();
        for ty in types {
            if matches!(ty, ast::Type::InferType(_)) {
                arguments.push(None);
                continue;
            }
            let tokens = match parsed.type_tokens(Some(ty.syntax().clone())) {
                Ok(tokens) => tokens,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            match crate::frontend::source_types::resolve_written_type(
                db, graph, &sources, signature, &tokens,
            ) {
                QueryPoll::Ready(Ok(Some(term))) => arguments.push(Some(OwnerArgument::Term(term))),
                QueryPoll::Ready(Ok(None)) => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "owner Type argument of a generated body needs further source Type resolution"
                            .into(),
                    )));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        QueryPoll::Ready(Ok(OwnerArguments {
            arguments,
            guards: Vec::new(),
        }))
    }

    /// The call of a path with owner Type arguments: the callee the ordinary
    /// call resolver selects for the path, with the owner's written Type
    /// bound to its impl's parameters.
    pub(super) fn select_owner_argument_call(
        &mut self,
        db: &mut Database,
        name: &str,
        syntax: &[String],
        owner: &OwnerArguments,
        argument_slots: &[SlotId],
    ) -> QueryPoll<SelectedArgumentCall> {
        let callee = match self.select_named_callee(db, name) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let bound = match self.owner_bindings(db, &callee, owner) {
            QueryPoll::Ready(Ok(bound)) => bound,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.prepare_selected_argument_call_with(db, callee, syntax, bound, argument_slots)
    }

    /// The owner's written Type arguments matched against the callee's impl
    /// self Type: each impl parameter they fix, with its runtime descriptor.
    fn owner_bindings(
        &mut self,
        db: &mut Database,
        callee: &SourceCallable,
        owner: &OwnerArguments,
    ) -> QueryPoll<Vec<(u32, ArgumentType)>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "source calls require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let signature = callee.signature();
        // The callee's `Self`: for an associated function of a generic impl
        // the impl's written self Type over its parameters.
        let pattern = match crate::frontend::source_types::resolve_written_type(
            db,
            graph,
            &sources,
            signature,
            &[crate::syntax::ast_adapter::TokenRecipe {
                kind: 0,
                text: "Self".into(),
            }],
        ) {
            QueryPoll::Ready(Ok(Some(pattern))) => pattern,
            // The owner's arguments only state what inference would find.
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(Vec::new())),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let TyTerm::Nominal(_, patterns) = &pattern else {
            return QueryPoll::Ready(Ok(Vec::new()));
        };
        // A hole `_` among the arguments leaves its parameter to inference.
        let patterns: Vec<&TyTerm> = patterns
            .iter()
            .filter_map(|argument| match argument {
                GenericArg::Type(ty) => Some(ty),
                _ => None,
            })
            .collect();
        let mut bound = Vec::new();
        for (position, argument) in owner.arguments.iter().enumerate() {
            let (Some(argument), Some(pattern)) = (argument, patterns.get(position)) else {
                continue;
            };
            match_owner(pattern, argument.source(), signature.definition, &mut bound);
        }
        let mut result = Vec::with_capacity(bound.len());
        for (ordinal, source) in bound {
            // A parameter bound to a whole written argument has that
            // argument's own descriptor; a part of one, a structural Type.
            let whole = owner
                .arguments
                .iter()
                .flatten()
                .find(|argument| *argument.source() == source);
            let descriptor = match whole {
                Some(argument) => match argument.descriptor(&self.frame, &mut self.store) {
                    Ok(descriptor) => descriptor,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                },
                None => match self.store.intern_structural_term(&source) {
                    Ok(Some(descriptor)) => descriptor,
                    Ok(None) => {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "owner Type argument binds a part of a written Type with no structural descriptor"
                                .into(),
                        )));
                    }
                    Err(error) => return QueryPoll::Ready(Err(error)),
                },
            };
            let source = self.frame.bound_view(&source, &self.store);
            result.push((ordinal, ArgumentType { source, descriptor }));
        }
        QueryPoll::Ready(Ok(result))
    }
}
