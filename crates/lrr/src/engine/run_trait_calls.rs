//! Trait-path calls: `Trait::name(..)`, `<Type as Trait>::name(..)`,
//! `<Type>::name(..)` and `T::name(..)` through an enclosing generic
//! parameter `T` (including a trait body's implicit `Self`). The function is
//! the one the run's impl member index selects for the call's self type —
//! the impl of the named trait for that type (its own definition, else the
//! trait's provided body, else an applicable blanket impl), as overloaded
//! operators and method calls select theirs. The selected function is then
//! an ordinary source call: the shared argument hints, contracts, packet,
//! frame entry and return cleanup.
//!
//! The self type is the anchor type, a generic parameter's reached binding,
//! or, for `Trait::name(..)`, the reached type of the argument the trait's
//! declaration writes as `Self` (a receiver, `Self`, `&Self` or `&mut Self`).
//! Such a call starts with the declaration's written signature and selects
//! its impl once that argument completes; a declaration whose parameters
//! never name `Self` (`Default::default()`) takes it from the expected
//! result.
use super::*;
use crate::{
    frontend::{callables::split_qualified_anchor, member_index::BuiltinSelf},
    runtime::shape::{TypeId, TypeKey},
    vm::frame::DeferredTraitCall,
};

/// An evaluated argument, as selecting among impls by the trait's arguments
/// sees it.
pub(super) enum SelectionArgument {
    /// Its reached source Type.
    Type(crate::traits::term::TyTerm),
    /// An unresolved numeric literal: rustc's integer (float) variable;
    /// `behind_reference` when the parameter takes it by shared reference
    /// (a comparison's `&Rhs`).
    Literal { integer: bool, behind_reference: bool },
    /// No closed Type: the argument does not discriminate.
    Unknown,
}

/// How a trait-path call proceeds at its first call opcode.
pub(super) enum TraitPathCall {
    /// The self type is known: the selected impl function.
    Selected(Arc<SourceCallable>),
    /// The self argument selects the impl once it is evaluated.
    Deferred {
        declaration: crate::core::digest::Digest,
        call: DeferredTraitCall,
    },
}

/// Whether written parameter tokens name `Self` itself (`Some(false)`) or a
/// reference to it (`Some(true)`); lifetimes are ignored.
fn self_parameter(tokens: &[crate::syntax::ast_adapter::TokenRecipe]) -> Option<bool> {
    let words: Vec<&str> = tokens
        .iter()
        .map(|token| token.text.as_str())
        .filter(|text| !text.starts_with('\''))
        .collect();
    match words.as_slice() {
        ["self"] | ["Self"] => Some(false),
        ["&", "self"] | ["&", "Self"] | ["&", "mut", "self"] | ["&", "mut", "Self"] => Some(true),
        _ => None,
    }
}

impl RunMachine {
    /// The member-index self type of a reached type: a nominal declaration,
    /// or a builtin self (primitive, `str`, slice, array, tuple, reference).
    pub(super) fn reached_self_type(
        &self,
        ty: TypeId,
    ) -> Result<Result<crate::core::digest::Digest, BuiltinSelf>, Diagnostic> {
        let key = self.store.type_descriptor(ty)?.key.clone();
        Ok(match key.as_ref() {
            TypeKey::Nominal { identity, .. } => Ok(*identity),
            TypeKey::Primitive(_) if ty.0 == 16 => Err(BuiltinSelf::Str),
            TypeKey::Primitive(_) => match self.store.descriptor_source_term(ty) {
                Some(crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)) => {
                    Err(BuiltinSelf::Tuple(0))
                }
                Some(crate::traits::term::TyTerm::Primitive(primitive)) => {
                    Err(BuiltinSelf::Primitive(primitive))
                }
                _ => return Err(call_fault("reached primitive self type is not selectable")),
            },
            TypeKey::Slice { .. } => Err(BuiltinSelf::Slice),
            TypeKey::Array { .. } => Err(BuiltinSelf::Array),
            TypeKey::Tuple(fields) => Err(BuiltinSelf::Tuple(
                u32::try_from(fields.len()).map_err(|_| call_fault("tuple arity overflow"))?,
            )),
            TypeKey::Ref { mutable, .. } => Err(BuiltinSelf::Reference(*mutable)),
            // E-R5-008: pointers select impls under their reserved identity.
            TypeKey::RawPtr { mutable: true, .. } => {
                Ok(crate::traits::pointer_terms::mut_pointer_identity())
            }
            TypeKey::RawPtr { mutable: false, .. } => {
                Ok(crate::traits::pointer_terms::const_pointer_identity())
            }
            TypeKey::Function { .. } => Ok(crate::traits::pointer_terms::fn_pointer_identity()),
            // A function item compares as the fn pointer it coerces to.
            TypeKey::Callable { captures, .. } if captures.is_empty() => {
                Ok(crate::traits::pointer_terms::fn_pointer_identity())
            }
            _ => {
                return Err(call_fault(
                    "self type awaits trait-object/closure impl selection",
                ));
            }
        })
    }
    /// The shared source callable of a member function the impl index
    /// selected (a method, trait-path or operator impl), selected once per
    /// run and revalidated at each later use.
    pub(super) fn member_callee(
        &mut self,
        db: &mut Database,
        function: crate::core::digest::Digest,
    ) -> QueryPoll<Arc<SourceCallable>> {
        if let Some(callee) = self.member_callees.get(&function) {
            if callee.revalidate(db).is_ok() {
                return QueryPoll::Ready(Ok(Arc::clone(callee)));
            }
            self.member_callees.remove(&function);
        }
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("member selection requires the impl index")));
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("member selection requires a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let callee = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver
                .with_members(Some(&members))
                .select_member_function(db, function),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        callee.map(|callee| {
            let callee: Arc<SourceCallable> = Arc::from(callee);
            self.member_callees.insert(function, Arc::clone(&callee));
            callee
        })
    }
    /// The reached binding of the caller's generic Type parameter `name`
    /// (`None` when the caller declares no such parameter).
    fn generic_binding(&self, db: &mut Database, name: &str) -> QueryPoll<Option<TypeId>> {
        let caller = self.frame.pc().body.definition;
        let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(caller)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("caller signature query variant mismatch")));
        };
        let Some(parameter) = signature.generics.iter().find(|parameter| {
            parameter.kind == crate::syntax::signatures::GenericKind::Type && parameter.name == name
        }) else {
            return QueryPoll::Ready(Ok(None));
        };
        let key = crate::traits::term::ParamKey {
            definition: caller,
            ordinal: parameter.ordinal,
        };
        QueryPoll::Ready(match self.frame.parameter_hint(key) {
            Ok(Some(binding)) => Ok(Some(binding)),
            Ok(None) => Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                format!("generic parameter `{name}` has no reached binding for its path call"),
            )),
            Err(error) => Err(error),
        })
    }
    /// Classify a written callee path as a trait-path call; `None` leaves
    /// it to ordinary path selection. That classification depends only on
    /// the written path and its caller, so an ordinary path is classified
    /// once per call site.
    pub(super) fn trait_path_call(
        &mut self,
        db: &mut Database,
        site: u32,
        name: &str,
        arguments: &[SlotId],
    ) -> QueryPoll<Option<TraitPathCall>> {
        if !name.contains("::") {
            return QueryPoll::Ready(Ok(None));
        }
        let key = (self.frame.pc().clone(), site);
        if self.ordinary_call_paths.contains(&key) {
            return QueryPoll::Ready(Ok(None));
        }
        let classified = self.classify_trait_path_call(db, name, arguments);
        if matches!(classified, QueryPoll::Ready(Ok(None))) {
            self.ordinary_call_paths.insert(key);
        }
        classified
    }
    fn classify_trait_path_call(
        &mut self,
        db: &mut Database,
        name: &str,
        arguments: &[SlotId],
    ) -> QueryPoll<Option<TraitPathCall>> {
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        if !name.contains("::") || name.starts_with('.') {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(Some(&members)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let caller = self.frame.pc().body.definition;
        // The written self type (anchor or generic owner), trait and name.
        let (self_written, trait_written, function) =
            if let Some((ty, tr, rest)) = split_qualified_anchor(name) {
                if rest.contains("::") {
                    return QueryPoll::Ready(Ok(None));
                }
                (Some(ty), tr, rest)
            } else {
                let Some((prefix, function)) = name.rsplit_once("::") else {
                    return QueryPoll::Ready(Ok(None));
                };
                if prefix.is_empty() {
                    return QueryPoll::Ready(Ok(None));
                }
                (None, Some(prefix), function)
            };
        // A single-name owner that is an enclosing generic parameter (a
        // trait body's implicit `Self` included) is its reached binding,
        // before any same-named module type.
        let owner = self_written.or(trait_written.filter(|prefix| !prefix.contains("::")));
        let generic = match owner {
            Some(owner) if !owner.contains("::") => poll!(self.generic_binding(db, owner)),
            _ => None,
        };
        let Some(binding) = generic else {
            // A static anchor (`<Sq as Shape>::f`, `<Sq>::f`) is resolved by
            // ordinary path selection (`CallResolver::anchored_function`);
            // `Trait::f` selects its impl from the reached self type.
            if self_written.is_some() {
                return QueryPoll::Ready(Ok(None));
            }
            let Some(tr) = trait_written else {
                return QueryPoll::Ready(Ok(None));
            };
            return match poll!(resolver.written_trait(db, caller, tr)) {
                Some(trait_key) => self.trait_function_call(db, trait_key, function, name, arguments),
                // `Type::f(args)` whose type has several impls of one trait
                // that differ by the trait's arguments (`u64::from(x)`):
                // the evaluated arguments select the impl.
                None if !arguments.is_empty() => {
                    match poll!(resolver.associated_alternatives(db, caller, name)) {
                        Some((trait_key, declaration, functions, self_term)) => {
                            let returns_self = match db
                                .fetch(&SyntaxProvider, QueryKey::FunctionSignature(declaration))
                            {
                                QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                                    QueryValue::FunctionSignature(signature) => {
                                        self_parameter(&signature.result) == Some(false)
                                    }
                                    _ => false,
                                },
                                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                            };
                            QueryPoll::Ready(Ok(Some(TraitPathCall::Deferred {
                                declaration,
                                call: DeferredTraitCall {
                                    trait_key,
                                    name: function.to_owned(),
                                    // Selected once the last argument is evaluated.
                                    self_ordinal: match u32::try_from(arguments.len() - 1) {
                                        Ok(ordinal) => ordinal,
                                        Err(_) => {
                                            return QueryPoll::Ready(Err(call_fault(
                                                "argument ordinal overflow",
                                            )));
                                        }
                                    },
                                    self_reference: false,
                                    arguments: arguments.to_vec(),
                                    alternatives: functions,
                                    compare_from: 0,
                                    result_self: self_term.filter(|_| returns_self),
                                },
                            })))
                        }
                        None => QueryPoll::Ready(Ok(None)),
                    }
                }
                None => QueryPoll::Ready(Ok(None)),
            };
        };
        let self_type = match self.reached_self_type(binding) {
            Ok(self_type) => self_type,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let candidates = match trait_written.filter(|_| self_written.is_some()) {
            // `<T as Trait>::f`: the named trait's impl for the binding.
            Some(tr) => {
                let Some(trait_key) = poll!(resolver.written_trait(db, caller, tr)) else {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-NAME-MISSING",
                        format!("qualified call path `{name}` names no trait"),
                    )));
                };
                members
                    .trait_method(self_type, trait_key, function)
                    .into_iter()
                    .map(|(_, function)| function)
                    .collect::<Vec<_>>()
            }
            // `T::f` / `<T>::f`: the trait impls of the binding (a generic
            // parameter has no inherent functions of its reached type).
            None => {
                // A reference self type: the referent selects among the
                // impls indexed together for `&U`.
                let pointee = match self.store.type_descriptor(binding).map(|d| d.key.clone()) {
                    Ok(key) => match key.as_ref() {
                        TypeKey::Ref { target, .. } => match self.reached_self_type(*target) {
                            Ok(pointee) => Some(pointee),
                            Err(_) => None,
                        },
                        _ => None,
                    },
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                members.trait_candidates_of(self_type, pointee, function)
            }
        };
        let selected = match candidates.as_slice() {
            [function] => *function,
            [] => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-MISSING",
                    format!("no impl provides `{function}` for the call's self type (`{name}`)"),
                )));
            }
            // Impls of one trait that differ by its arguments (`Join<&str>`,
            // `Join<&[T]>`): the call starts against the trait's declaration
            // and the evaluated arguments select the impl.
            _ if !arguments.is_empty()
                && members.same_trait_of(&candidates).is_some_and(|trait_key| {
                    members.trait_declaration(trait_key, function).is_some()
                }) =>
            {
                let trait_key = members.same_trait_of(&candidates).expect("checked");
                let declaration = members
                    .trait_declaration(trait_key, function)
                    .expect("checked");
                // A declaration with a `Self`/`&Self` parameter takes the
                // impl from it, so only the later arguments select among the
                // trait's argument variants; one without (`fn from(value: T)
                // -> Self`) compares every argument and constrains the
                // impl's result to the reached Self.
                let (has_self_parameter, returns_self) = match db
                    .fetch(&SyntaxProvider, QueryKey::FunctionSignature(declaration))
                {
                    QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                        QueryValue::FunctionSignature(signature) => (
                            signature
                                .parameters
                                .iter()
                                .any(|parameter| self_parameter(&parameter.ty).is_some()),
                            self_parameter(&signature.result) == Some(false),
                        ),
                        _ => (true, false),
                    },
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let result_self = if returns_self && !has_self_parameter {
                    self.store.descriptor_source_term(binding)
                } else {
                    None
                };
                return QueryPoll::Ready(Ok(Some(TraitPathCall::Deferred {
                    declaration,
                    call: DeferredTraitCall {
                        trait_key,
                        name: function.to_owned(),
                        // Selected once the last argument is evaluated.
                        self_ordinal: match u32::try_from(arguments.len() - 1) {
                            Ok(ordinal) => ordinal,
                            Err(_) => {
                                return QueryPoll::Ready(Err(call_fault(
                                    "argument ordinal overflow",
                                )));
                            }
                        },
                        self_reference: false,
                        arguments: arguments.to_vec(),
                        alternatives: candidates,
                        compare_from: usize::from(has_self_parameter),
                        result_self,
                    },
                })));
            }
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-AMBIGUOUS",
                    format!(
                        "several impls provide `{function}` for the call's self type (`{name}`); selection by the other argument types awaits impl argument matching"
                    ),
                )));
            }
        };
        self.member_callee(db, selected)
            .map(|callee| Some(TraitPathCall::Selected(callee)))
    }
    /// `Trait::name(..)`: the declaration's parameter written as `Self`
    /// selects the impl once evaluated; with none, the expected result
    /// (a declared `-> Self`) does.
    fn trait_function_call(
        &mut self,
        db: &mut Database,
        trait_key: crate::core::digest::Digest,
        function: &str,
        name: &str,
        arguments: &[SlotId],
    ) -> QueryPoll<Option<TraitPathCall>> {
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(declaration) = members.trait_declaration(trait_key, function) else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-NAME-MISSING",
                format!("trait path `{name}` names no function its trait declares"),
            )));
        };
        let signature = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(declaration)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("trait declaration signature mismatch")));
        };
        if let Some((ordinal, reference)) = signature
            .parameters
            .iter()
            .enumerate()
            .find_map(|(ordinal, parameter)| {
                self_parameter(&parameter.ty).map(|reference| (ordinal, reference))
            })
            && ordinal < arguments.len()
        {
            let mut slots = Vec::new();
            slots.extend_from_slice(arguments);
            return QueryPoll::Ready(Ok(Some(TraitPathCall::Deferred {
                declaration,
                call: DeferredTraitCall {
                    trait_key,
                    name: function.to_owned(),
                    self_ordinal: match u32::try_from(ordinal) {
                        Ok(ordinal) => ordinal,
                        Err(_) => return QueryPoll::Ready(Err(call_fault("argument ordinal overflow"))),
                    },
                    self_reference: reference,
                    arguments: slots,
                    alternatives: Vec::new(),
                    compare_from: 1,
                    result_self: None,
                },
            })));
        }
        // `-> Self` with no `Self` parameter: the expected result type.
        let result_self = self_parameter(&signature.result) == Some(false);
        let result_slot = self.frame.body.nodes()[self.frame.pc().node as usize].value;
        let expected = match result_slot.map(|slot| self.call_result_source_hint(slot)).transpose() {
            Ok(expected) => expected.flatten(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (true, Some(expected)) = (result_self, expected) else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                format!(
                    "trait path call `{name}` needs its Self type from an argument or the expected result"
                ),
            )));
        };
        let self_type = match self.reached_self_type(expected.descriptor) {
            Ok(self_type) => self_type,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        self.select_trait_impl_function(db, self_type, trait_key, function, name)
            .map(|callee| Some(TraitPathCall::Selected(callee)))
    }
    /// `<self_type as Trait>::function` through the member index.
    fn select_trait_impl_function(
        &mut self,
        db: &mut Database,
        self_type: Result<crate::core::digest::Digest, BuiltinSelf>,
        trait_key: crate::core::digest::Digest,
        function: &str,
        name: &str,
    ) -> QueryPoll<Arc<SourceCallable>> {
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("trait path calls require the impl index")));
        };
        let selected = match members.trait_method(self_type, trait_key, function).as_slice() {
            [(_, function)] => *function,
            [] => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-MISSING",
                    format!("no impl of the named trait provides `{function}` for the call's self type (`{name}`)"),
                )));
            }
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-AMBIGUOUS",
                    format!(
                        "several impls provide `{function}` for the call's self type (`{name}`); selection by the other argument types awaits impl argument matching"
                    ),
                )));
            }
        };
        self.member_callee(db, selected)
    }
    /// The call's first opcode reached a deferred trait-path call: record it
    /// and start the call against the trait's declaration, whose written
    /// parameters give its arguments no hint before the impl is selected.
    pub(super) fn start_deferred_trait_call(
        &mut self,
        site: u32,
        declaration: crate::core::digest::Digest,
        call: DeferredTraitCall,
        arguments: &[SlotId],
    ) {
        let identity = self.frame.call_identity(site);
        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
            ordinary: crate::frontend::callables::OrdinaryCallWitness::trait_declaration(
                self.frame.body.body().clone(),
                NodeId(site),
                declaration,
            ),
            site,
            hints: arguments.iter().map(|slot| (*slot, None, None)).collect(),
            partial: Vec::new(),
        });
        self.frame.trait_calls.insert(identity, call);
    }
    /// The deferred call's self argument completed (`evaluated` written
    /// arguments are evaluated): select the impl from its reached type and
    /// publish the selected call, whose remaining arguments then take their
    /// ordinary parameter hints.
    pub(super) fn select_deferred_trait_call(
        &mut self,
        db: &mut Database,
        site: u32,
        evaluated: usize,
    ) -> QueryPoll<()> {
        let identity = self.frame.call_identity(site);
        let Some(deferred) = self.frame.trait_calls.get(&identity).cloned() else {
            return QueryPoll::Ready(Err(call_fault("deferred trait call record absent")));
        };
        let callee = if deferred.alternatives.is_empty() {
            let ordinal = deferred.self_ordinal as usize;
            let reached = (|| -> Result<TypeId, Diagnostic> {
                let slot = *deferred.arguments.get(ordinal).ok_or_else(|| {
                    call_fault("trait call self argument is outside the written call")
                })?;
                let cell = self.frame.cell(slot)?;
                // An unsuffixed literal self argument takes rustc's fallback.
                self.store.bind_literal_fallback(cell)?;
                let value = self.store.admit_move(cell)?;
                let ty = self.store.effective_type(value)?.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "trait call self argument requires its reached concrete type".into(),
                    )
                })?;
                if !deferred.self_reference {
                    return Ok(ty);
                }
                match self.store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Ref { target, .. } => Ok(*target),
                    _ => Err(Diagnostic::new(
                        "LRR-E-CALL-ARG-TYPE",
                        "trait call self argument is not the declared reference to `Self`".into(),
                    )),
                }
            })();
            let reached = match reached {
                Ok(reached) => reached,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let self_type = match self.reached_self_type(reached) {
                Ok(self_type) => self_type,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            // Several impls of the trait for the self type, differing by the
            // trait's arguments (`PartialEq<[U]>` and `PartialEq<Vec<U>>` of
            // `[T]`): the later arguments select once they are evaluated.
            let several = match &self.members {
                Some(members) => {
                    let mut functions = Vec::new();
                    for (_, function) in
                        members.trait_method(self_type, deferred.trait_key, &deferred.name)
                    {
                        if !functions.contains(&function) {
                            functions.push(function);
                        }
                    }
                    (functions.len() > 1).then_some(functions)
                }
                None => None,
            };
            if let Some(functions) = several {
                if evaluated < deferred.arguments.len() {
                    // Not selectable yet: wait for the last argument.
                    let record = self
                        .frame
                        .trait_calls
                        .get_mut(&identity)
                        .ok_or_else(|| call_fault("deferred trait call record absent"));
                    match record {
                        Ok(record) => {
                            record.alternatives = functions;
                            record.self_ordinal = match u32::try_from(deferred.arguments.len() - 1) {
                                Ok(ordinal) => ordinal,
                                Err(_) => {
                                    return QueryPoll::Ready(Err(call_fault(
                                        "argument ordinal overflow",
                                    )));
                                }
                            };
                        }
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    }
                    return QueryPoll::Ready(Ok(()));
                }
                let mut views = Vec::with_capacity(deferred.arguments.len());
                for slot in &deferred.arguments {
                    match self.selection_argument(*slot) {
                        Ok(view) => views.push(view),
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    }
                }
                let function = match self.select_by_arguments(db, &functions, 1, &views, None) {
                    QueryPoll::Ready(Ok(Some(function))) => function,
                    QueryPoll::Ready(Ok(None)) => {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-NAME-AMBIGUOUS",
                            format!(
                                "several impls provide `{}` for the receiver and its argument types",
                                deferred.name
                            ),
                        )));
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                match self.member_callee(db, function) {
                    QueryPoll::Ready(Ok(callee)) => callee,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            } else {
                match self.select_trait_impl_function(
                    db,
                    self_type,
                    deferred.trait_key,
                    &deferred.name,
                    &deferred.name,
                ) {
                    QueryPoll::Ready(Ok(callee)) => callee,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
        } else {
            // Impls of one trait differing by its arguments: every argument
            // is evaluated; their reached types select the impl.
            let mut views = Vec::with_capacity(deferred.arguments.len());
            let mut types = Vec::with_capacity(deferred.arguments.len());
            for slot in &deferred.arguments {
                match self.selection_argument_typed(*slot) {
                    Ok((view, ty)) => {
                        views.push(view);
                        types.push(ty);
                    }
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            // The selection is fixed per (site, reached argument Types);
            // an unresolved literal argument is part of the key as `None`.
            let key = (self.frame.pc().clone(), types);
            let function = match self.argument_selections.get(&key) {
                Some(function) => *function,
                None => {
                    let function =
                        match self.select_by_arguments(
                            db,
                            &deferred.alternatives,
                            deferred.compare_from,
                            &views,
                            deferred.result_self.as_ref(),
                        ) {
                            QueryPoll::Ready(Ok(Some(function))) => function,
                            QueryPoll::Ready(Ok(None)) => {
                                return QueryPoll::Ready(Err(Diagnostic::new(
                                    "LRR-E-NAME-AMBIGUOUS",
                                    format!(
                                        "several impls provide `{}` for the receiver and its argument types",
                                        deferred.name
                                    ),
                                )));
                            }
                            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                        };
                    self.argument_selections.insert(key, function);
                    function
                }
            };
            match self.member_callee(db, function) {
                QueryPoll::Ready(Ok(callee)) => callee,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
        let selected =
            match self.selected_after_arguments(db, callee, &deferred.arguments, evaluated) {
                QueryPoll::Ready(Ok(selected)) => selected,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        self.pending_bytes = self.pending_bytes.saturating_add(selected.bytes);
        self.pending.insert(identity, selected);
        self.frame.trait_calls.remove(&identity);
        QueryPoll::Ready(Ok(()))
    }
    /// An evaluated call argument as impl selection sees it.
    pub(super) fn selection_argument(
        &mut self,
        slot: SlotId,
    ) -> Result<SelectionArgument, Diagnostic> {
        self.selection_argument_typed(slot).map(|(argument, _)| argument)
    }
    /// `selection_argument` with the argument's reached runtime Type (`None`
    /// for an unresolved literal), which keys the selection.
    fn selection_argument_typed(
        &mut self,
        slot: SlotId,
    ) -> Result<(SelectionArgument, Option<TypeId>), Diagnostic> {
        let cell = self.frame.cell(slot)?;
        let value = self.store.admit_move(cell)?;
        Ok(match self.store.effective_type(value)? {
            Some(descriptor) => {
                let view = match self.frame.source_view(slot)? {
                    Some(view) => Some(self.frame.bound_view(&view, &self.store)),
                    None => self.store.descriptor_source_term(descriptor),
                };
                (
                    view.map_or(SelectionArgument::Unknown, SelectionArgument::Type),
                    Some(descriptor),
                )
            }
            // An unresolved literal is rustc's integer (float) variable.
            None if matches!(value.kind, 11 | 12) => (
                SelectionArgument::Literal {
                    integer: value.kind == 11,
                    behind_reference: false,
                },
                None,
            ),
            None => (SelectionArgument::Unknown, None),
        })
    }
    /// rustc's impl selection by the trait's arguments: of several impls
    /// for one self type (`impl PartialEq<i32> for M` beside
    /// `impl PartialEq<M> for M`), the one whose parameters (from `first`)
    /// accept the arguments, through the shared signature substitution's
    /// unification. An unresolved literal unifies only with an integer
    /// (float) type or a parameter, and, when several still accept it,
    /// rustc's `i32`/`f64` fallback decides. `None` when no single impl
    /// accepts the arguments.
    pub(super) fn select_by_arguments(
        &mut self,
        db: &mut Database,
        candidates: &[crate::core::digest::Digest],
        first: usize,
        arguments: &[SelectionArgument],
        result_self: Option<&crate::traits::term::TyTerm>,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        use crate::{ir::op::PrimitiveType as P, traits::term::TyTerm};
        // Each accepting impl, and whether every unresolved literal it
        // accepts takes exactly rustc's fallback type there.
        let mut accepted: Vec<(crate::core::digest::Digest, bool)> = Vec::new();
        for function in candidates {
            let callee = match self.member_callee(db, *function) {
                QueryPoll::Ready(Ok(callee)) => callee,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let hints = match self.operator_parameter_hints(db, &callee) {
                QueryPoll::Ready(Ok(hints)) => hints,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let mut signature = callee.signature().clone();
            let mut compared = Vec::new();
            let mut fallback = true;
            let mut fits = true;
            for (ordinal, argument) in arguments.iter().enumerate().skip(first) {
                // A parameter without a source template cannot be compared.
                let Some(Some(hint)) = hints.get(ordinal) else {
                    continue;
                };
                match argument {
                    SelectionArgument::Unknown => {}
                    SelectionArgument::Type(view) => {
                        let Some(parameter) = signature.parameters.get_mut(ordinal) else {
                            continue;
                        };
                        parameter.symbolic = Some(hint.source.clone());
                        compared.push((ordinal, view));
                    }
                    SelectionArgument::Literal {
                        integer,
                        behind_reference,
                    } => {
                        let parameter = match (&hint.source, behind_reference) {
                            (TyTerm::Ref(_, false, referent), true) => referent.as_ref(),
                            (parameter, false) => parameter,
                            _ => {
                                fits = false;
                                break;
                            }
                        };
                        match (parameter, integer) {
                            (TyTerm::Param(_) | TyTerm::Infer(_), _) => {}
                            (TyTerm::Primitive(P::F32 | P::F64), false) => {
                                fallback &= *parameter == TyTerm::Primitive(P::F64);
                            }
                            (
                                TyTerm::Primitive(
                                    P::Unit | P::Bool | P::Char | P::F32 | P::F64,
                                ),
                                _,
                            ) => {
                                fits = false;
                                break;
                            }
                            (TyTerm::Primitive(_), true) => {
                                fallback &= *parameter == TyTerm::Primitive(P::I32);
                            }
                            _ => {
                                fits = false;
                                break;
                            }
                        }
                    }
                }
            }
            if !fits {
                continue;
            }
            let accepts = crate::frontend::signature_subst::SignatureSubstitution::new(
                &signature, *function,
            )
            .and_then(|mut substitution| {
                for (ordinal, view) in &compared {
                    substitution.constrain_argument_erased(*ordinal, view)?;
                }
                // A `-> Self` function of a type-relative call: the impl's
                // result is the written Self.
                if let Some(self_term) = result_self
                    && substitution.source_result().is_some()
                {
                    substitution.constrain_result(self_term)?;
                }
                Ok(())
            })
            .is_ok();
            if accepts {
                accepted.push((*function, fallback));
            }
        }
        QueryPoll::Ready(Ok(match accepted.as_slice() {
            [(function, _)] => Some(*function),
            // Several accept an unresolved literal: the fallback type's.
            several => {
                let mut by_fallback = several.iter().filter(|(_, fallback)| *fallback);
                match (by_fallback.next(), by_fallback.next()) {
                    (Some((function, _)), None)
                        if arguments
                            .iter()
                            .any(|argument| matches!(argument, SelectionArgument::Literal { .. })) =>
                    {
                        Some(*function)
                    }
                    _ => None,
                }
            }
        }))
    }
    /// A source call selected after `evaluated` of its written arguments:
    /// those arguments keep their reached types (a parameter whose written
    /// Type is not a closed source goal binds structurally from its argument,
    /// as the operator dispatcher's), and later arguments take their
    /// ordinary hints as they are reached.
    fn selected_after_arguments(
        &mut self,
        db: &mut Database,
        callee: Arc<SourceCallable>,
        arguments: &[SlotId],
        evaluated: usize,
    ) -> QueryPoll<SelectedArgumentCall> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("trait path calls require a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let parameters = callee.signature().parameters.len();
        let capacity = parameters.min(arguments.len());
        let mut hints = Vec::with_capacity(capacity);
        for ordinal in 0..evaluated.min(capacity) {
            let hint = match crate::frontend::call_contracts::prepare_argument_hint(
                db, &callee, graph, &sources, ordinal,
            ) {
                QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(mut prepared)))
                    if prepared.len() == 1 =>
                {
                    prepared.pop().expect("checked single hint")
                }
                QueryPoll::Ready(Ok(_)) => {
                    let reached = (|| -> Result<ArgumentType, Diagnostic> {
                        let slot = arguments[ordinal];
                        let cell = self.frame.cell(slot)?;
                        let value = self.store.admit_move(cell)?;
                        let descriptor = self.store.effective_type(value)?.ok_or_else(|| {
                            Diagnostic::new(
                                "LRR-E-TYPE-CONTEXT",
                                "call argument requires a reached concrete type".into(),
                            )
                        })?;
                        let view = self
                            .frame
                            .argument_source_view(slot, descriptor, &self.store)?
                            .map(|view| self.frame.bound_view(&view, &self.store));
                        reached_argument_type(descriptor, view)
                    })();
                    match reached {
                        Ok(reached) => ArgumentHint {
                            closed_reference: None,
                            source: reached.source,
                            descriptor: Some(reached.descriptor),
                            written: None,
                        },
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    }
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            hints.push(hint);
        }
        let result_slot = self
            .frame
            .runtime_call
            .as_ref()
            .map(|call| call.result())
            .or(self.frame.body.nodes()[self.frame.pc().node as usize].value);
        let result_hint = match result_slot
            .map(|slot| self.call_result_source_hint(slot))
            .transpose()
        {
            Ok(hint) => hint.flatten(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        for hint in &mut hints {
            argument_hints::apply_context(hint, &callee, &[], result_hint.as_ref());
        }
        let mut bytes = callee
            .signature()
            .estimated_bytes()
            .saturating_add(hints.capacity().saturating_mul(std::mem::size_of::<ArgumentHint>()))
            .saturating_add(arguments.len().saturating_mul(std::mem::size_of::<SlotId>()));
        for hint in &hints {
            match argument_hints::retained_hint_bytes(hint) {
                Ok(heap) => bytes = bytes.saturating_add(heap),
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        }
        QueryPoll::Ready(Ok(SelectedArgumentCall {
            callee,
            explicit: Vec::new(),
            typed: None,
            hints,
            argument_slots: arguments.to_vec(),
            result_hint,
            expected: std::collections::BTreeMap::new(),
            bytes,
        }))
    }
}
