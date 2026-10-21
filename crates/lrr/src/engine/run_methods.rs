//! Method calls: select the method from the receiver's reached type through
//! the run's impl member index (inherent impls first, then trait impls, as
//! rustc's probe for a nominal receiver), and record how the receiver
//! argument is formed. The call then proceeds as an ordinary source call.
use super::*;
use crate::{
    runtime::shape::TypeKey,
    vm::frame::{ReceiverAdjustment, ReceiverMode, ReceiverStep},
};

impl RunMachine {
    pub(super) fn select_method_call(
        &mut self,
        db: &mut Database,
        site: u32,
        name: &str,
        syntax: &[String],
        arguments: &[SlotId],
        receiver_place: SlotId,
    ) -> QueryPoll<MethodCall> {
        let method = name.strip_prefix('.').unwrap_or(name);
        // Autoderef the receiver's reached type to its nominal declaration.
        let mut ty = match self.frame.place_type(receiver_place, &self.store) {
            Ok(ty) => ty,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        use crate::frontend::member_index::BuiltinSelf;
        let mut steps: Vec<ReceiverStep> = Vec::new();
        // Mutability of each reference stripped by autoderef, outermost first
        // (`None` for a builtin box dereference or an overloaded step).
        let mut references = Vec::new();
        // The mutability of an autoref the overloaded-deref probe picked: the
        // method belongs to an impl for `&U` / `&mut U` taking `self` by value
        // (rustc's autoref step on the dereferenced type).
        let mut autoref: Option<bool> = None;
        // Each autoderef step's reached type with the receiver steps taken to
        // reach it, for an explicit `self: Rc<Self>` / `self: &Rc<Self>`
        // receiver type that names one of them.
        let mut step_types: Vec<(crate::runtime::shape::TypeId, usize)> = Vec::new();
        let reference_sources: Vec<_> = self.sources.iter().collect();
        let reference_choice = match (
            self.members.as_ref(),
            self.store.descriptor_source_term(ty),
            self.graph.as_ref(),
        ) {
            (Some(members), Some(term), Some(graph)) => {
                match crate::frontend::reference_methods::probe(
                    db,
                    members,
                    graph,
                    &reference_sources,
                    &term,
                    method,
                ) {
                    QueryPoll::Ready(Ok(choice)) => choice,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            _ => None,
        };
        let self_type = if let Some(choice) = &reference_choice {
            for _ in 0..choice.depth {
                step_types.push((ty, steps.len()));
                let key = match self.store.type_descriptor(ty) {
                    Ok(descriptor) => descriptor.key.clone(),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                let TypeKey::Ref { target, mutable } = key.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault(
                        "selected reference receiver depth differs",
                    )));
                };
                steps.push(ReceiverStep::Deref);
                references.push(Some(*mutable));
                ty = *target;
            }
            step_types.push((ty, steps.len()));
            choice.self_type
        } else {
            loop {
                step_types.push((ty, steps.len()));
                let key = match self.store.type_descriptor(ty) {
                    Ok(descriptor) => descriptor.key.clone(),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                match key.as_ref() {
                    TypeKey::Ref { target, mutable } => {
                        steps.push(ReceiverStep::Deref);
                        references.push(Some(*mutable));
                        ty = *target;
                        // rustc's probe tries each step's own methods before the
                        // next deref: a reference to a reference (`&&A`) has the
                        // methods of core's `&A` impls (`Self = &A`) at this step.
                        if let Ok(descriptor) = self.store.type_descriptor(ty)
                            && let TypeKey::Ref { mutable: inner, .. } = descriptor.key.as_ref()
                            && self.members.as_ref().is_some_and(|members| {
                                !members
                                    .builtin_trait_functions(BuiltinSelf::Reference(*inner), method)
                                    .is_empty()
                            })
                        {
                            break Err(BuiltinSelf::Reference(*inner));
                        }
                    }
                    // rustc's probe steps through a box by its builtin deref when
                    // `Box<T>` itself has no method of this name.
                    TypeKey::Nominal { identity, args, .. }
                        if self.store.owned_box() == Some(*identity)
                            && args.len() == 1
                            && self.members.as_ref().is_some_and(|members| {
                                members
                                    .select_method(Ok(*identity), method)
                                    .is_err_and(|error| error.code == "LRR-E-NAME-MISSING")
                            }) =>
                    {
                        steps.push(ReceiverStep::Deref);
                        references.push(None);
                        ty = args[0];
                    }
                    TypeKey::Nominal { identity, .. } => {
                        // This step's own methods first; a type with none steps
                        // through its `Deref` impl (rustc's overloaded autoderef).
                        let missing = self.members.as_ref().is_some_and(|members| {
                            members
                                .method_alternatives(Ok(*identity), method)
                                .is_err_and(|error| error.code == "LRR-E-NAME-MISSING")
                        });
                        if !missing {
                            break Ok(*identity);
                        }
                        match self.overloaded_deref_probe(db, ty, method) {
                            QueryPoll::Ready(Ok(Some((chain, bucket, by_reference)))) => {
                                references.extend(chain.iter().map(|_| None));
                                steps.extend(chain);
                                autoref = by_reference;
                                break bucket;
                            }
                            QueryPoll::Ready(Ok(None)) => break Ok(*identity),
                            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                        }
                    }
                    TypeKey::Primitive(_) if ty.0 == 16 => break Err(BuiltinSelf::Str),
                    TypeKey::Primitive(_) => match self.store.descriptor_source_term(ty) {
                        Some(crate::traits::term::TyTerm::Primitive(primitive)) => {
                            break Err(BuiltinSelf::of_primitive(primitive));
                        }
                        _ => {
                            return QueryPoll::Ready(Err(call_fault(&format!(
                                "method `{method}` receiver primitive is not selectable"
                            ))));
                        }
                    },
                    TypeKey::Slice { .. } => break Err(BuiltinSelf::Slice),
                    TypeKey::Array { .. } => break Err(BuiltinSelf::Array),
                    // Raw and fn pointers select impls under their reserved
                    // identities (E-R5-008).
                    TypeKey::RawPtr { mutable: true, .. } => {
                        break Ok(crate::traits::pointer_terms::mut_pointer_identity());
                    }
                    TypeKey::RawPtr { mutable: false, .. } => {
                        break Ok(crate::traits::pointer_terms::const_pointer_identity());
                    }
                    TypeKey::Function { .. } => {
                        break Ok(crate::traits::pointer_terms::fn_pointer_identity());
                    }
                    TypeKey::Tuple(fields) => match u32::try_from(fields.len()) {
                        Ok(arity) => break Err(BuiltinSelf::Tuple(arity)),
                        Err(_) => return QueryPoll::Ready(Err(call_fault("tuple arity overflow"))),
                    },
                    _ => {
                        return QueryPoll::Ready(Err(call_fault(&format!(
                            "method `{method}` on this receiver type awaits trait-object/closure methods"
                        ))));
                    }
                }
            }
        };
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("method selection requires the impl index")));
        };
        // The shared probe rule (inherent, trait, blanket impls; the array
        // unsizing step, whose receiver is the autoref'd element window).
        let mut self_type = self_type;
        // The probe also tries `Self = &U` at the step of the reference it
        // peeled last: a method of an impl for `&'a U` (autoref'd `&&U`, or a
        // by-value `self`) when `U` itself has none.
        let mut self_is_reference = reference_choice
            .as_ref()
            .is_some_and(|choice| choice.reference_self);
        let selected = if let Some(choice) = &reference_choice {
            Ok(choice.alternatives.clone())
        } else {
            match autoref {
                Some(mutable) => {
                    let pointee = self_type;
                    self_type = Err(BuiltinSelf::Reference(mutable));
                    self_is_reference = true;
                    members.method_alternatives_of(self_type, Some(pointee), method)
                }
                None => members.method_alternatives(self_type, method),
            }
        };
        let alternatives = match selected {
            Ok(alternatives) => alternatives,
            Err(error) if error.code == "LRR-E-NAME-MISSING" => {
                match (references.last().copied().flatten(), steps.last()) {
                    (Some(mutable), Some(ReceiverStep::Deref)) => {
                        match members.method_alternatives_of(
                            Err(BuiltinSelf::Reference(mutable)),
                            Some(self_type),
                            method,
                        ) {
                            Ok(alternatives) => {
                                steps.pop();
                                references.pop();
                                self_type = Err(BuiltinSelf::Reference(mutable));
                                self_is_reference = true;
                                alternatives
                            }
                            Err(_) => return QueryPoll::Ready(Err(error)),
                        }
                    }
                    _ => return QueryPoll::Ready(Err(error)),
                }
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let unsize = alternatives.unsize;
        // Impls of one trait share its declaration. The call starts against
        // that declaration and selects its impl by all evaluated arguments,
        // including the receiver's actual Self application even when the
        // method has no other arguments (`select_deferred_trait_call`).
        let deferred = match (alternatives.functions.as_slice(), alternatives.same_trait) {
            ([_], _) => None,
            (_, Some(trait_key)) if syntax.is_empty() => members
                .trait_declaration(trait_key, method)
                .map(|declaration| (trait_key, declaration)),
            _ => None,
        };
        let function = match (alternatives.functions.as_slice(), &deferred) {
            ([function], _) => *function,
            (functions, Some(_)) => functions[0],
            // Inherent impls of different instantiations: the receiver's
            // reached type arguments select.
            (functions, None) if alternatives.inherent => {
                match self.select_inherent_by_receiver(db, functions, ty) {
                    QueryPoll::Ready(Ok(Some(function))) => function,
                    QueryPoll::Ready(Ok(None)) => {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-NAME-AMBIGUOUS",
                            format!(
                                "method `{method}` has several inherent definitions for the receiver's type"
                            ),
                        )));
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-AMBIGUOUS",
                    format!("method `{method}` is provided by several trait impls"),
                )));
            }
        };
        let callee = match self.member_callee(db, function) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let Some(receiver) = callee
            .signature()
            .parameters
            .first()
            .filter(|parameter| parameter.receiver)
        else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                format!("`{method}` is an associated function, not a method"),
            )));
        };
        let receiver_form = crate::frontend::reference_methods::receiver_form(&receiver.ty);
        let mut mode = match receiver_form {
            crate::frontend::reference_methods::Form::Shared => ReceiverMode::Shared,
            crate::frontend::reference_methods::Form::Mutable => ReceiverMode::Mutable,
            crate::frontend::reference_methods::Form::Value => ReceiverMode::Value,
            crate::frontend::reference_methods::Form::Explicit => {
                match receiver.ty.first().map(|token| token.text.as_str()) {
                    Some("&") if receiver.ty.iter().any(|token| token.text == "mut") => {
                        ReceiverMode::Mutable
                    }
                    Some("&") => ReceiverMode::Shared,
                    _ => ReceiverMode::Value,
                }
            }
        };
        // An explicit receiver type other than `Self`, `&Self`, `&mut Self`
        // (`self: Rc<Self>`, `self: &Rc<Self>`): rustc's probe matches it
        // against the type of one autoderef step, by value or autoref'd.
        let receiver_tokens = receiver.ty.clone();
        if receiver_form == crate::frontend::reference_methods::Form::Explicit {
            match self.arbitrary_receiver(db, &callee, &receiver_tokens, &step_types) {
                QueryPoll::Ready(Ok(Some((taken, adjusted)))) => {
                    steps.truncate(taken);
                    references.truncate(taken);
                    mode = adjusted;
                    self_is_reference = true;
                    autoref = None;
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        // A by-value `self` of an impl for `&U` / `&mut U` reached through the
        // overloaded-deref probe takes the autoref of the dereferenced place.
        if let Some(mutable) = autoref
            && mode == ReceiverMode::Value
        {
            mode = if mutable {
                ReceiverMode::Mutable
            } else {
                ReceiverMode::Shared
            };
        }
        // `&self` through a shared reference passes that reference itself
        // (a copy), rather than dereferencing and reborrowing it.
        if unsize && mode == ReceiverMode::Value {
            return QueryPoll::Ready(Err(call_fault(&format!(
                "method `{method}` takes an unsized slice receiver by value"
            ))));
        }
        if !unsize
            && !self_is_reference
            && mode == ReceiverMode::Shared
            && references.last() == Some(&Some(false))
        {
            steps.pop();
            mode = ReceiverMode::Value;
        }
        // A mutable autoref reaches its referent through `DerefMut`.
        if mode == ReceiverMode::Mutable {
            for step in &mut steps {
                if let ReceiverStep::Overloaded { mutable } = step {
                    *mutable = true;
                }
            }
        }
        self.frame.method_receivers.insert(
            self.frame.call_identity(site),
            ReceiverAdjustment {
                steps,
                mode,
                unsize,
            },
        );
        if let Some((trait_key, declaration)) = deferred {
            return QueryPoll::Ready(Ok(MethodCall::Deferred {
                declaration,
                call: crate::vm::frame::DeferredTraitCall {
                    trait_key,
                    name: method.to_owned(),
                    self_ordinal: match u32::try_from(arguments.len() - 1) {
                        Ok(ordinal) => ordinal,
                        Err(_) => {
                            return QueryPoll::Ready(Err(call_fault("argument ordinal overflow")));
                        }
                    },
                    self_type: None,
                    arguments: arguments.to_vec(),
                    alternatives: alternatives.functions,
                    result_self: None,
                    expected: Vec::new(),
                },
            }));
        }
        self.prepare_selected_argument_call(db, callee, syntax, arguments)
            .map(MethodCall::Selected)
    }

}

/// The self-type bucket of a closed source type, as the impl index keys it.
fn probe_bucket(
    term: &crate::traits::term::TyTerm,
) -> Option<Result<crate::core::digest::Digest, crate::frontend::member_index::BuiltinSelf>> {
    use crate::frontend::member_index::BuiltinSelf;
    use crate::traits::term::TyTerm;
    Some(match term {
        TyTerm::Nominal(identity, _) => Ok(*identity),
        TyTerm::Str => Err(BuiltinSelf::Str),
        TyTerm::Slice(_) => Err(BuiltinSelf::Slice),
        TyTerm::Array(..) => Err(BuiltinSelf::Array),
        TyTerm::Primitive(primitive) => Err(BuiltinSelf::of_primitive(*primitive)),
        _ => return None,
    })
}

impl RunMachine {
    /// rustc's overloaded autoderef: the receiver type `ty` has no method
    /// `method` of its own, so the probe steps through its `Deref` impl
    /// (`<T as Deref>::Target`, normalized by the shared associated-type
    /// normalizer over the impl index), peeling any references of the target,
    /// until a type provides the method. The chain's steps (overloaded steps
    /// not yet picking `DerefMut`) and the providing type's impl bucket, or
    /// `None` when the type has no `Deref` impl or the chain provides no
    /// such method.
    pub(super) fn overloaded_deref_probe(
        &mut self,
        db: &mut Database,
        ty: crate::runtime::shape::TypeId,
        method: &str,
    ) -> QueryPoll<
        Option<(
            Vec<ReceiverStep>,
            Result<crate::core::digest::Digest, crate::frontend::member_index::BuiltinSelf>,
            Option<bool>,
        )>,
    > {
        use crate::traits::term::{ProjectionKey, TraitRef, TyTerm};
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Ok(None));
        };
        let deref = match self.operator_trait(db, "ops", "Deref") {
            QueryPoll::Ready(Ok(deref)) => deref,
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let receiver_trait = match self.operator_trait(db, "ops", "Receiver") {
            QueryPoll::Ready(Ok(receiver)) => Some(receiver),
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => None,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let Some(mut term) = self.store.descriptor_source_term(ty) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut steps = Vec::new();
        // rustc's autoderef recursion limit.
        for _ in 0..128 {
            let Some(Ok(head)) = probe_bucket(&term) else {
                return QueryPoll::Ready(Ok(None));
            };
            // `Deref` steps call `deref`; an unstable `Receiver` impl names
            // the next step's type only (arbitrary self types), with no call.
            let mut by_receiver = false;
            let mut stepping = deref;
            if members.trait_method(Ok(head), deref, "deref").is_empty() {
                let Some(receiver) = receiver_trait else {
                    return QueryPoll::Ready(Ok(None));
                };
                if !members.implements(Ok(head), receiver) {
                    return QueryPoll::Ready(Ok(None));
                }
                by_receiver = true;
                stepping = receiver;
            }
            let projection = TyTerm::Projection(Box::new(ProjectionKey {
                trait_ref: TraitRef {
                    trait_key: stepping,
                    self_ty: term.clone(),
                    args: Vec::new(),
                },
                item: crate::frontend::member_index::associated_name_key("Target"),
                item_args: Vec::new(),
            }));
            let target = match crate::frontend::associated_types::normalize(
                db, graph, &sources, &members, &projection,
            ) {
                QueryPoll::Ready(Ok(Some(target))) => target,
                QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if !by_receiver {
                steps.push(ReceiverStep::Overloaded { mutable: false });
            }
            let mut target = target;
            while let TyTerm::Ref(_, _, inner) = target {
                steps.push(ReceiverStep::Deref);
                target = *inner;
            }
            let Some(bucket) = probe_bucket(&target) else {
                return QueryPoll::Ready(Ok(None));
            };
            if members.method_alternatives(bucket, method).is_ok() {
                return QueryPoll::Ready(Ok(Some((steps, bucket, None))));
            }
            // rustc's autoref step on this type: a by-value method of an
            // impl for `&U` / `&mut U` (`impl Foo for &[isize]`).
            for mutable in [false, true] {
                if members
                    .method_alternatives_of(
                        Err(crate::frontend::member_index::BuiltinSelf::Reference(mutable)),
                        Some(bucket),
                        method,
                    )
                    .is_ok()
                {
                    return QueryPoll::Ready(Ok(Some((steps, bucket, Some(mutable)))));
                }
            }
            term = target;
        }
        QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-RECURSION-LIMIT",
            format!("reached the recursion limit while auto-dereferencing for method `{method}`"),
        )))
    }
}

impl RunMachine {
    /// rustc's probe of a method with an explicit receiver type: the first
    /// autoderef step whose type the declared receiver type is (by value),
    /// or whose reference it is (autoref `&` / `&mut`). Returns the number
    /// of receiver steps to take and the receiver mode, or `None` when no
    /// step's type is named (the impl bucket's own adjustment then stands).
    fn arbitrary_receiver(
        &mut self,
        db: &mut Database,
        callee: &SourceCallable,
        tokens: &[crate::syntax::ast_adapter::TokenRecipe],
        step_types: &[(crate::runtime::shape::TypeId, usize)],
    ) -> QueryPoll<Option<(usize, ReceiverMode)>> {
        use crate::traits::term::TyTerm;
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let declared = match crate::frontend::source_types::resolve_written_type(
            db,
            graph,
            &sources,
            callee.signature(),
            tokens,
        ) {
            QueryPoll::Ready(Ok(Some(declared))) => declared,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        for (ty, taken) in step_types {
            match self.instantiates(&declared, *ty) {
                Ok(true) => return QueryPoll::Ready(Ok(Some((*taken, ReceiverMode::Value)))),
                Ok(false) => {}
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
            if let TyTerm::Ref(_, mutable, inner) = &declared {
                match self.instantiates(inner, *ty) {
                    Ok(true) => {
                        let mode = if *mutable { ReceiverMode::Mutable } else { ReceiverMode::Shared };
                        return QueryPoll::Ready(Ok(Some((*taken, mode))));
                    }
                    Ok(false) => {}
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
        }
        QueryPoll::Ready(Ok(None))
    }
    /// Of inherent methods of impls for different instantiations of one
    /// type, the one whose impl self type the reached `receiver` type
    /// instantiates (`Foo<usize>` against `impl Foo<usize>` and
    /// `impl Foo<isize>`). `None` unless exactly one impl does.
    fn select_inherent_by_receiver(
        &mut self,
        db: &mut Database,
        functions: &[crate::core::digest::Digest],
        receiver: crate::runtime::shape::TypeId,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        let mut accepted = Vec::new();
        for function in functions {
            let callee = match self.member_callee(db, *function) {
                QueryPoll::Ready(Ok(callee)) => callee,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let Some(graph) = &self.graph else {
                return QueryPoll::Ready(Err(call_fault("method selection requires a catalogue")));
            };
            let sources: Vec<_> = self.sources.iter().collect();
            let pattern = match crate::frontend::source_types::resolve_written_type(
                db,
                graph,
                &sources,
                callee.signature(),
                &[crate::syntax::ast_adapter::TokenRecipe {
                    kind: 0,
                    text: "Self".into(),
                }],
            ) {
                QueryPoll::Ready(Ok(Some(pattern))) => pattern,
                // An impl self type outside the written-Type service does
                // not discriminate.
                QueryPoll::Ready(Ok(None)) => {
                    accepted.push(*function);
                    continue;
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match self.instantiates(&pattern, receiver) {
                Ok(true) => accepted.push(*function),
                Ok(false) => {}
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        }
        QueryPoll::Ready(Ok(match accepted.as_slice() {
            [function] => Some(*function),
            _ => None,
        }))
    }
    /// Whether the reached type `ty` is an instance of the impl self type
    /// `pattern` (its type parameters match anything).
    fn instantiates(
        &mut self,
        pattern: &crate::traits::term::TyTerm,
        ty: crate::runtime::shape::TypeId,
    ) -> Result<bool, Diagnostic> {
        use crate::traits::term::{GenericArg, TyTerm};
        let key = self.store.type_descriptor(ty)?.key.clone();
        Ok(match (pattern, key.as_ref()) {
            (TyTerm::Param(_), _) => true,
            (TyTerm::Nominal(identity, patterns), TypeKey::Nominal { identity: actual, args, .. }) => {
                if identity != actual {
                    return Ok(false);
                }
                let args = args.to_vec();
                let mut position = 0;
                for pattern in patterns {
                    if let GenericArg::Type(pattern) = pattern {
                        let Some(arg) = args.get(position) else {
                            return Ok(false);
                        };
                        position += 1;
                        if !self.instantiates(pattern, *arg)? {
                            return Ok(false);
                        }
                    }
                }
                true
            }
            (TyTerm::Ref(_, mutable, pattern), TypeKey::Ref { mutable: actual, target }) => {
                mutable == actual && self.instantiates(pattern, *target)?
            }
            (TyTerm::Slice(pattern), TypeKey::Slice { element }) => self.instantiates(pattern, *element)?,
            (TyTerm::Array(pattern, _), TypeKey::Array { element, .. }) => {
                self.instantiates(pattern, *element)?
            }
            (TyTerm::Tuple(patterns), TypeKey::Tuple(fields)) => {
                let fields = fields.to_vec();
                if patterns.len() != fields.len() {
                    return Ok(false);
                }
                for (pattern, field) in patterns.iter().zip(fields) {
                    if !self.instantiates(pattern, field)? {
                        return Ok(false);
                    }
                }
                true
            }
            // A closed primitive or `str`: the descriptor it names.
            (pattern, _) => self.store.find_structural_term(pattern) == Some(ty),
        })
    }
}

/// How a method call proceeds at its first call opcode.
pub(super) enum MethodCall {
    /// The probe selected one function.
    Selected(SelectedArgumentCall),
    /// Several impls of one trait: the evaluated arguments select among
    /// them.
    Deferred {
        declaration: crate::core::digest::Digest,
        call: crate::vm::frame::DeferredTraitCall,
    },
}
