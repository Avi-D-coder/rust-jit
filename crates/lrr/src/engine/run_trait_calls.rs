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
            None => members.trait_candidates(self_type, function),
        };
        let selected = match candidates.as_slice() {
            [function] => *function,
            [] => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-MISSING",
                    format!("no impl provides `{function}` for the call's self type (`{name}`)"),
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
        let ordinal = deferred.self_ordinal as usize;
        let reached = (|| -> Result<TypeId, Diagnostic> {
            let slot = *deferred
                .arguments
                .get(ordinal)
                .ok_or_else(|| call_fault("trait call self argument is outside the written call"))?;
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
        let callee = match self.select_trait_impl_function(
            db,
            self_type,
            deferred.trait_key,
            &deferred.name,
            &deferred.name,
        ) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
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
                        let view = match self.frame.source_view(slot)? {
                            Some(view) => Some(self.frame.bound_view(&view, &self.store)),
                            None => self.store.descriptor_source_term(descriptor),
                        };
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
            monomorphic: true,
        }))
    }
}
