//! Overloaded operators, dispatcher side. A reached operator whose operands
//! are not builtin primitives demands its lang-item trait method (see
//! `vm/frame_operator_calls.rs`). The dispatcher selects that method from
//! the self operand's reached type through the run's impl member index — the
//! impl of the trait whose definition the operator's lang item names, else
//! the trait's provided body, else an applicable blanket impl — and enters it
//! as an ordinary source call: the shared contract preparation, argument
//! packet, frame entry and return cleanup of every source call. The call
//! resumes no recipe call site: its result becomes the suspended operation's
//! result and the operation completes.
use super::*;
use crate::{
    frontend::names::NameLookup, runtime::cell::TicketId, source::manifest::Namespace,
    vm::frame::operator_calls_frame::OperatorDemand,
};

/// An operator's selected trait method: the shared callee and each
/// parameter's closed reference contract and descriptor, as an ordinary
/// call's argument receives it. A parameter whose written Type is not a
/// closed source goal (`&Self` of a generic impl) has none: it binds
/// structurally from its argument in the contracts.
pub(super) struct OperatorMethod {
    callee: Arc<SourceCallable>,
    hints: Vec<Option<ArgumentHint>>,
}

impl RunMachine {
    pub(super) fn operator_parameter_hints(
        &self,
        db: &mut Database,
        callee: &SourceCallable,
    ) -> QueryPoll<Vec<Option<ArgumentHint>>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("operator call requires a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut hints = Vec::new();
        for ordinal in 0..callee.signature().parameters.len() {
            hints.push(
                match crate::frontend::call_contracts::prepare_argument_hint(
                    db, callee, graph, &sources, ordinal,
                ) {
                    QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(mut hint)))
                        if hint.len() == 1 =>
                    {
                        hint.pop()
                    }
                    QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(_))) => {
                        return QueryPoll::Ready(Err(call_fault(
                            "operator method parameter hint differs from its parameter",
                        )));
                    }
                    QueryPoll::Ready(Ok(_)) => None,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                },
            );
        }
        QueryPoll::Ready(Ok(hints))
    }
    pub(super) fn enter_operator_call(&mut self, db: &mut Database) -> QueryPoll<()> {
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
        let Some(demand) = self.frame.operator_demand() else {
            return QueryPoll::Ready(Err(call_fault("no operator method is demanded")));
        };
        // A borrow's deref coercion steps only along a `Deref` chain that
        // reaches the Type the borrow expects (never an unsizing or a
        // trait-object conversion); otherwise the borrow runs as written.
        if let Some((place_type, expected, mutable)) =
            ready!(self.frame.pending_coercion_goal(&self.store))
        {
            match self.deref_chain_reaches(db, place_type, &expected, mutable) {
                QueryPoll::Ready(Ok(true)) => {}
                QueryPoll::Ready(Ok(false)) => {
                    return QueryPoll::Ready(self.frame.decline_coercion(place_type));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        if self.prepared_call.is_some() || self.frame.call_arguments.is_some() {
            return QueryPoll::Ready(Err(call_fault(
                "an operator method cannot interleave a sealed call",
            )));
        }
        let right = ready!(self.frame.operator_right_type(&self.store));
        let key = (self.frame.pc().clone(), demand.kind.key(), demand.self_type, right);
        // The selection and its parameter hints are fixed per (site,
        // operator, self type, right operand type); later executions share
        // them.
        let method = match self.operator_methods.get(&key) {
            Some(method) => {
                ready!(method.callee.revalidate(db));
                Arc::clone(method)
            }
            None => {
                let callee = poll!(self.select_operator_method(db, &demand));
                let hints = poll!(self.operator_parameter_hints(db, &callee));
                let method = Arc::new(OperatorMethod { callee, hints });
                self.operator_methods.insert(key, Arc::clone(&method));
                method
            }
        };
        let hints = &method.hints;
        // A closed parameter Type types an unresolved literal operand.
        let mut expected = Vec::with_capacity(hints.len());
        for hint in hints {
            expected.push(match hint {
                Some(hint) => match hint.descriptor {
                    Some(descriptor) => Some(descriptor),
                    None => ready!(self.store.intern_structural_term(&hint.source)),
                },
                None => None,
            });
        }
        let formed = ready!(self
            .frame
            .form_operator_arguments(&expected, &mut self.store));
        if formed.len() != hints.len() {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "operator method parameters differ from its operands".into(),
            )));
        }
        // The method was selected for the operands' reached types: they cross
        // the boundary in their monomorphic views.
        let arguments: Vec<_> = formed
            .iter()
            .map(|argument| {
                let view = self.frame.bound_view(&argument.source, &self.store);
                // A view naming a declared `dyn Trait` position crosses the
                // boundary as the value's concrete runtime Type.
                let source = if view.contains_dyn() {
                    self.store
                        .descriptor_source_term(argument.descriptor)
                        .filter(|term| !term.contains_dyn())
                        .unwrap_or(view)
                } else {
                    view
                };
                ArgumentType {
                    source,
                    descriptor: argument.descriptor,
                }
            })
            .collect();
        // Only closed reference parameters consume their actual evidence.
        let mut references = Vec::new();
        if arguments
            .iter()
            .any(|argument| matches!(argument.source, crate::traits::term::TyTerm::Ref(..)))
        {
            for (ordinal, argument) in formed.iter().enumerate() {
                let closed = hints[ordinal]
                    .as_ref()
                    .is_some_and(|hint| hint.closed_reference.is_some());
                references.push(
                    if closed && matches!(argument.source, crate::traits::term::TyTerm::Ref(..)) {
                        Some(ready!(match argument.slot {
                            // An operand slot retains its reference's source
                            // view; one bound by a pattern from a tuple of
                            // references may not, and then the operand's own
                            // reached type states it, as for a formed reference.
                            Some(slot)
                                if self
                                    .frame
                                    .source_view_ref(slot)
                                    .is_ok_and(|view| view.is_some()) =>
                                ClosedReferenceArgument::capture_budgeted(
                                    &self.frame,
                                    slot,
                                    &self.store,
                                    usize::MAX,
                                ),
                            _ => ClosedReferenceArgument::capture_formed(
                                &self.store,
                                argument.cell,
                                &argument.source,
                            ),
                        }))
                    } else {
                        None
                    },
                );
            }
        }
        let result_hint = match demand.destination {
            Some(destination) => ready!(self.call_result_source_hint(destination)),
            None => None,
        }
        .map(|mut hint| {
            hint.source = self.frame.bound_view(&hint.source, &self.store);
            hint
        });
        let call = poll!(self.prepare_selected_source_call(
            db,
            Arc::clone(&method.callee),
            &[],
            result_hint.as_ref(),
            &arguments,
            &references,
        ));
        let cells: Vec<_> = formed.iter().map(|argument| argument.cell).collect();
        QueryPoll::Ready((|| {
            if let Some(proof) = &call.contracts.closed_references {
                proof.require_argument_cells(&self.store, &cells, &arguments)?;
            }
            call.callee.revalidate(db)?;
            let args = crate::vm::call::ArgumentPacket::seal(
                &mut self.store,
                &cells,
                &call.contracts.parameter_descriptors,
            )?;
            let callee = self.frame.enter_operator_call(
                call.body.clone(),
                call.body.pc(NodeId(0), Phase::Entry)?,
                &mut self.store,
            )?;
            self.enter_callee(call, callee, &args, CallContinuation::Operator)
        })())
    }
    /// The lang-item trait's definition, as the operator's own frame names
    /// `core::<module>::<Trait>` (through `std` where `core` is not in scope).
    pub(super) fn operator_trait(
        &mut self,
        db: &mut Database,
        module: &str,
        name: &'static str,
    ) -> QueryPoll<crate::core::digest::Digest> {
        if let Some(trait_key) = self.operator_traits.get(name) {
            return QueryPoll::Ready(Ok(*trait_key));
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("operator trait requires a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let caller = self.frame.pc().body.definition;
        for krate in ["core", "std"] {
            match resolver.lookup_written(
                db,
                caller,
                &format!("::{krate}::{module}::{name}"),
                Namespace::Type,
            ) {
                QueryPoll::Ready(Ok(NameLookup::Defined(definition))) => {
                    self.operator_traits.insert(name, definition.identity);
                    return QueryPoll::Ready(Ok(definition.identity));
                }
                QueryPoll::Ready(Ok(_)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-NAME-MISSING",
            format!("operator trait `core::{module}::{name}` is not in the run's library"),
        )))
    }
    /// `<Self as Trait>::method` of the demanded operator through the member
    /// index and the ordinary call resolver.
    fn select_operator_method(
        &mut self,
        db: &mut Database,
        demand: &OperatorDemand,
    ) -> QueryPoll<Arc<SourceCallable>> {
        let (module, trait_name, method) = demand.kind.lang_item();
        let trait_key = match self.operator_trait(db, module, trait_name) {
            QueryPoll::Ready(Ok(key)) => key,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let self_type = match self.reached_self_type(demand.self_type) {
            Ok(self_type) => self_type,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("operator selection requires the impl index")));
        };
        let mut candidates = members.trait_method(self_type, trait_key, method);
        // `impl Trait for &Vec<T>` vs `&[T]` vs a generic `&A`: reference self
        // types are indexed together, so the referent selects among them. An
        // impl naming the referent wins over one generic in it.
        let referent = match self.store.type_descriptor(demand.self_type) {
            Ok(descriptor) => match descriptor.key.as_ref() {
                crate::runtime::shape::TypeKey::Ref { target, .. } => Some(*target),
                _ => None,
            },
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if candidates.len() > 1
            && let Some(target) = referent
            && let Ok(pointee) = self.reached_self_type(target)
        {
            let exact: Vec<_> = candidates
                .iter()
                .filter(|(members, _)| members.self_pointee == Some(pointee))
                .cloned()
                .collect();
            let generic: Vec<_> = candidates
                .iter()
                .filter(|(members, _)| members.self_pointee.is_none())
                .cloned()
                .collect();
            // An impl naming the referent wins over one generic in it, when
            // its `Rhs` accepts the right operand (`impl PartialEq<Vec<U>>
            // for &[T]` does not select `&[T] == &[T]`).
            let exact_accepts = if exact.is_empty() || generic.is_empty() {
                true
            } else {
                let mut functions = Vec::new();
                for (_, function) in &exact {
                    if !functions.contains(function) {
                        functions.push(*function);
                    }
                }
                match self.select_by_right_operand(db, &functions) {
                    QueryPoll::Ready(Ok(accepted)) => accepted.is_some(),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
            candidates = if exact.is_empty() || !exact_accepts {
                generic
            } else {
                exact
            };
        }
        let function = match candidates.as_slice() {
            [(_, function)] => *function,
            [] => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-MISSING",
                    format!("no `{trait_name}` impl provides `{method}` for the operand's type"),
                )));
            }
            several => {
                // Impls that differ only by the trait's arguments share the
                // trait's provided body (`PartialEq::ne` of `PartialEq<i32>`
                // and `PartialEq<M>`): one function, whose own calls select
                // their impls by the reached argument types.
                let mut functions = Vec::new();
                for (_, function) in several {
                    if !functions.contains(function) {
                        functions.push(*function);
                    }
                }
                let chosen = match functions.as_slice() {
                    [function] => QueryPoll::Ready(Ok(Some(*function))),
                    _ => self.select_by_right_operand(db, &functions),
                };
                match chosen {
                    QueryPoll::Ready(Ok(Some(function))) => function,
                    QueryPoll::Ready(Ok(None)) => {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-NAME-AMBIGUOUS",
                            format!(
                                "several `{trait_name}` impls provide `{method}` for the operand's type and its right operand"
                            ),
                        )));
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
        };
        let callee = match self.member_callee(db, function) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let receiver = callee
            .signature()
            .parameters
            .first()
            .is_some_and(|parameter| parameter.receiver);
        if !receiver {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-SIGNATURE",
                format!("selected `{trait_name}::{method}` is not a method"),
            )));
        }
        QueryPoll::Ready(Ok(callee))
    }
    /// rustc's impl selection by the trait's argument: of several impls for
    /// the self type (`impl PartialEq<String> for &str` beside
    /// `impl PartialEq<&B> for &A`), the one whose `Rhs` parameter accepts
    /// the right operand's argument (`select_by_arguments`). `None` when no
    /// single impl accepts it.
    fn select_by_right_operand(
        &mut self,
        db: &mut Database,
        candidates: &[crate::core::digest::Digest],
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        use super::trait_calls::SelectionArgument;
        let right = match self.frame.operator_right_view(&self.store) {
            Ok(view) => view.map(|view| self.frame.bound_view(&view, &self.store)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let right = match right {
            Some(view) => SelectionArgument::Type(view),
            None => match self.frame.operator_right_literal(&self.store) {
                Ok(Some(integer)) => SelectionArgument::Literal {
                    integer,
                    behind_reference: self.frame.operator_comparison(),
                },
                Ok(None) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            },
        };
        self.select_by_arguments(db, candidates, 1, &[SelectionArgument::Unknown, right], None)
    }
    /// The returned value is admitted as any source call's result, at the
    /// suspended operation's destination.
    pub(super) fn admit_operator_return(
        &self,
        db: &mut Database,
        suspended: &mut SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        // A place operation's `Deref` step returns into a dispatcher-owned
        // location, with no frame slot expectation to check.
        if suspended.caller.operator_is_place_deref() {
            return self.admit_call_result_at(db, suspended, ticket, None);
        }
        let destination = suspended
            .caller
            .operator_result_slot()
            .ok_or_else(|| call_fault("operator result requires an entered method"))?;
        self.admit_call_result(db, suspended, ticket, destination)?;
        suspended.caller.preflight_operator_result(ticket, &self.store)
    }
    /// The method frame finished its return cleanup: restore the operator's
    /// frame as any call's caller and complete its suspended operation.
    pub(super) fn resume_operator_call(
        &mut self,
        suspended: SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        if !matches!(suspended.continuation, CallContinuation::Operator) {
            return Err(call_fault("return continuation is not an operator call"));
        }
        let (_, source) = self.restore_caller(suspended, ticket)?;
        self.frame.finish_operator_call(ticket, source, &mut self.store)
    }
}
