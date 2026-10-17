//! Reached nominal constructor values and calls (unit/tuple structs and enum
//! variants), selected through the same `CallResolver::source_function`
//! Value-namespace route as source callees and closed facade functions.
//!
//! Selection and schema binding are cold, once per exact PC (value paths) or
//! call site; the bound variant witness is retained immutably and installed as
//! frame facts before dispatch. The VM constructs the aggregate (see
//! `vm/frame_nominal_values.rs`); no name resolution happens during dispatch.
use super::*;
use crate::{
    frontend::{
        nominal_constructors::{BoundConstructor, NominalConstructorTarget, bind_constructor},
        types::FieldShape,
    },
    vm::frame::nominal_values_frame::construct_from_arguments,
};

/// Run-wide immutable selections. Keys are exact source PCs/call sites of the
/// current source publication; the run faults on any revision change.
#[derive(Default)]
pub(super) struct NominalRunFacts {
    values: BTreeMap<PcKey, Arc<BoundConstructor>>,
    /// `None` records a call site whose callee is not a constructor.
    calls: BTreeMap<(BodyKey, crate::vm::call::CallIdentity), Option<Arc<BoundConstructor>>>,
    /// Positional members (`t.0`) keyed by PC and the base's nominal identity.
    positional:
        BTreeMap<(PcKey, crate::core::digest::Digest), crate::frontend::types::BoundNominalField>,
}

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

impl RunMachine {
    /// Dispatcher facts for the nominal operations of the installed recipe at
    /// this PC: a `ValuePath` unit constructor and a positional member
    /// projection whose executed base has a retained nominal source view.
    pub(super) fn prepare_nominal_operations(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        let operation = match self.installed.get_current(db, id) {
            Ok(recipe) => {
                let operations = &recipe.recipe().operations;
                let resume = self.frame.resume_operation_index().min(operations.len());
                operations[resume..]
                    .iter()
                    .enumerate()
                    .find_map(|(position, op)| match op {
                        // The Type the recipe itself states for the value
                        // before the path is read (`SetHint` of its own
                        // result, ahead of this operation) is its context.
                        Op::ValuePath { path, destination } => Some(Ok((
                            path.clone(),
                            operations[resume..resume + position]
                                .iter()
                                .find_map(|earlier| match earlier {
                                    Op::SetHint {
                                        child,
                                        hint: crate::ir::op::HintRecipe::Written(written),
                                    } if child == destination => Some(Arc::clone(written)),
                                    _ => None,
                                }),
                        ))),
                        Op::ProjectTuple {
                            base,
                            field,
                            destination,
                        } => Some(Err((*base, *field, *destination))),
                        _ => None,
                    })
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match operation {
            None => QueryPoll::Ready(Ok(())),
            Some(Ok((path, stated))) => self.prepare_value_path(db, &path, stated.as_deref()),
            Some(Err((base, field, destination))) => {
                // A box's own field is its heap cell, reached only by the
                // builtin deref: `b.0` autoderefs through the box.
                if let Err(failure) = self.frame.autoderef_box_member_base(base, &self.store) {
                    return QueryPoll::Ready(Err(match failure {
                        crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                        crate::runtime::place::PlaceFailure::GuestPanic(panic) => {
                            Diagnostic::new("LRR-E-INTERNAL-IR", panic.message())
                        }
                    }));
                }
                // Structural tuple bases keep their own projection route.
                let Ok(definition) = self.frame.nominal_place_source(base) else {
                    return QueryPoll::Ready(Ok(()));
                };
                self.prepare_positional_field(db, base, field, destination, definition)
            }
        }
    }
    fn prepare_positional_field(
        &mut self,
        db: &mut Database,
        base: SlotId,
        field: u32,
        destination: SlotId,
        definition: crate::core::digest::Digest,
    ) -> QueryPoll<()> {
        let key = (self.frame.pc().clone(), definition);
        if let Some(binding) = self.nominal.positional.get(&key) {
            return QueryPoll::Ready(self.frame.prepare_source_field(
                base,
                destination,
                binding.clone(),
            ));
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("field requires a checked source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let owner = match crate::frontend::source_body_context::capture(
            db,
            &sources,
            &self.frame.body,
            self.frame.pc(),
        ) {
            QueryPoll::Ready(Ok(owner)) => owner,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition)) {
            QueryPoll::Ready(Ok(captured)) => captured,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("positional field schema outcome differs")));
        };
        let names = match crate::frontend::types::SchemaNames::from_current_database(
            db, graph, &sources, schema,
        ) {
            QueryPoll::Ready(Ok(names)) => names,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if names.definition().item_kind != crate::source::manifest::ItemKind::Struct {
            return QueryPoll::Ready(Err(unsupported(
                "an enum value's fields are reached through its patterns",
            )));
        }
        let binding =
            match names.bind_positional_field_current(db, &sources, None, field, owner.scope()) {
                QueryPoll::Ready(Ok(crate::frontend::types::NominalFieldBinding::Ready(
                    binding,
                ))) => binding,
                QueryPoll::Ready(Ok(_)) => {
                    return QueryPoll::Ready(Err(unsupported(
                        "field expression requires further selected source type or generic binding",
                    )));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        self.nominal.positional.insert(key, binding.clone());
        QueryPoll::Ready(self.frame.prepare_source_field(base, destination, binding))
    }
    /// Select `path` through the ordinary Value namespace from the executing
    /// body's declaration context. `None` is any non-constructor selection.
    pub(super) fn select_nominal_constructor(
        &self,
        db: &mut Database,
        path: &str,
    ) -> QueryPoll<Option<BoundConstructor>> {
        self.select_nominal_constructor_with(db, path, false)
    }
    /// With `probe`, a callee whose Value-namespace resolution fails is simply
    /// not a constructor: the ordinary source-call route then reports it (or
    /// selects it through its entered lexical/macro contexts).
    fn select_nominal_constructor_with(
        &self,
        db: &mut Database,
        path: &str,
        probe: bool,
    ) -> QueryPoll<Option<BoundConstructor>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "value paths require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let target: NominalConstructorTarget =
            match resolver.source_function(db, self.frame.pc().body.definition, path) {
                QueryPoll::Ready(Ok(CallSelection::Constructor(target))) => target,
                QueryPoll::Ready(Ok(CallSelection::Unresolved(lookup))) if !probe => {
                    use crate::frontend::names::NameLookup;
                    return QueryPoll::Ready(Err(match lookup {
                        NameLookup::Inaccessible(_) => Diagnostic::new(
                            "LRR-E-NAME-VISIBILITY",
                            "value path selects an inaccessible item".into(),
                        ),
                        NameLookup::Ambiguous(_) | NameLookup::AmbiguousModules(_) => {
                            Diagnostic::new(
                                "LRR-E-NAME-AMBIGUOUS",
                                "value path is ambiguous".into(),
                            )
                        }
                        _ => unsupported("value path requires further namespace resolution"),
                    }));
                }
                QueryPoll::Ready(Ok(_)) => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(_)) if probe => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(mut error)) => {
                    if error.code == "LRR-E-NAME-MISSING" {
                        error.message = format!("{} (value path `{path}`)", error.message);
                    }
                    return QueryPoll::Ready(Err(error));
                }
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        bind_constructor(db, graph, &sources, &target).map(Some)
    }
    /// Install the unit constructor selected for the `ValuePath` at this PC.
    pub(super) fn prepare_value_path(
        &mut self,
        db: &mut Database,
        path: &str,
        stated: Option<&crate::ir::written_type::WrittenType>,
    ) -> QueryPoll<()> {
        let pc = self.frame.pc().clone();
        if self.item_values.contains_key(&pc) {
            return self.prepare_item_value_path(db, path);
        }
        // A static read by value: its selection is cached per PC; only the
        // cell's current contents are read on each execution.
        if let Some((cell, view)) = self.static_reads.get(&pc).cloned() {
            return QueryPoll::Ready(self.read_static_value(cell, view));
        }
        // Lexical (body-local) scope precedes the module graph.
        match self.prepare_lexical_value_path(db, path) {
            QueryPoll::Ready(Ok(true)) => return QueryPoll::Ready(Ok(())),
            QueryPoll::Ready(Ok(false)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let bound = if let Some(bound) = self.nominal.values.get(&pc) {
            Arc::clone(bound)
        } else {
            let bound = match self.select_nominal_constructor(db, path) {
                QueryPoll::Ready(Ok(Some(bound))) => bound,
                // Constants, statics and function items are Value items too.
                QueryPoll::Ready(Ok(None)) => return self.prepare_item_value_path(db, path),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match bound.shape() {
                FieldShape::Unit => {}
                FieldShape::Tuple => {
                    return QueryPoll::Ready(Err(unsupported(
                        "tuple constructor function value awaits function-value integration",
                    )));
                }
                FieldShape::Record => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-NAME-CONTEXT",
                        "record struct or variant is not a value; use its record constructor"
                            .into(),
                    )));
                }
            }
            let bound = Arc::new(bound);
            self.nominal.values.insert(pc, Arc::clone(&bound));
            bound
        };
        QueryPoll::Ready(self.frame.prepare_nominal_value(
            Arc::clone(bound.variant()),
            stated,
            &mut self.store,
        ))
    }
    /// Select (once per call site) whether this call names a tuple struct or
    /// tuple variant constructor. An unqualified callee that requires its
    /// entered lexical context keeps the ordinary source-call route.
    pub(super) fn select_constructor_call(
        &mut self,
        db: &mut Database,
        site: u32,
        name: &str,
    ) -> QueryPoll<bool> {
        let identity = self.frame.call_identity(site);
        if self.frame.selected_constructor_call(site).is_some() {
            return QueryPoll::Ready(Ok(true));
        }
        // Method calls (`.name`) are selected from their receiver's type.
        if self.pending.contains_key(&identity) || name.starts_with('.') {
            return QueryPoll::Ready(Ok(false));
        }
        let key = (self.frame.pc().body.clone(), identity);
        // The classification of a call site is fixed for the run's source
        // publication; ordinary calls are probed once, not per execution.
        let bound = if let Some(cached) = self.nominal.calls.get(&key) {
            match cached {
                Some(bound) => Arc::clone(bound),
                None => return QueryPoll::Ready(Ok(false)),
            }
        } else {
            if !name.contains("::") {
                match self.requires_lexical_callee_context(db, name) {
                    QueryPoll::Ready(Ok(true)) => {
                        self.nominal.calls.insert(key, None);
                        return QueryPoll::Ready(Ok(false));
                    }
                    QueryPoll::Ready(Ok(false)) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            let bound = match self.select_nominal_constructor_with(db, name, true) {
                QueryPoll::Ready(Ok(Some(bound))) => bound,
                QueryPoll::Ready(Ok(None)) => {
                    self.nominal.calls.insert(key, None);
                    return QueryPoll::Ready(Ok(false));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match bound.shape() {
                FieldShape::Tuple => {}
                FieldShape::Unit => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "a unit struct or unit variant is not a callable constructor".into(),
                    )));
                }
                FieldShape::Record => {
                    return QueryPoll::Ready(Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "a record struct or variant is constructed with braces, not called".into(),
                    )));
                }
            }
            let bound = Arc::new(bound);
            self.nominal.calls.insert(key, Some(Arc::clone(&bound)));
            bound
        };
        self.frame
            .select_constructor_call(site, Arc::clone(bound.variant()));
        QueryPoll::Ready(Ok(true))
    }
    /// Constructor arguments take their declared field Types as hints, so a
    /// written literal is typed by its field exactly as by a callee parameter.
    pub(super) fn start_constructor_call(
        &mut self,
        site: u32,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        let bound = self
            .frame
            .selected_constructor_call(site)
            .cloned()
            .ok_or_else(|| call_fault("constructor call start lacks its selection"))?;
        if arguments.len() != bound.fields().len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "written argument count differs from the constructor fields".into(),
            ));
        }
        // A generic declaration's parameter-typed fields are hinted only when
        // the call's result context already fixes its type arguments.
        let result = self
            .frame
            .body
            .nodes()
            .get(site as usize)
            .and_then(|node| node.value)
            .ok_or_else(|| call_fault("constructor call result slot absent"))?;
        let args = self.frame.nominal_hint_args(result, &bound, &self.store)?;
        // A partial result context (`Result<_, u32>`) fixes some arguments.
        let seeded = match &args {
            Some(_) => None,
            None => self
                .frame
                .partial_nominal_args(result, &bound, &mut self.store)?,
        };
        let mut hints = Vec::new();
        hints
            .try_reserve_exact(arguments.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for (slot, field) in arguments.iter().zip(bound.fields()) {
            // A field naming one of the declaration's parameters is hinted
            // only once the call's context fixes the type arguments.
            let descriptor = match (&args, &seeded) {
                (Some(args), _) => Some(field.materialize(&mut self.store, args)?),
                (None, Some(seeded)) if field.generic() => {
                    field.materialize_partial(&mut self.store, seeded)?
                }
                (None, _) if field.generic() => None,
                (None, _) => Some(field.materialize(&mut self.store, &[])?),
            };
            let source = match field.concrete().map(|source| source.source()) {
                Some(source @ crate::traits::term::TyTerm::Primitive(_))
                | Some(source @ crate::traits::term::TyTerm::Nominal(_, _)) => Some(source.clone()),
                _ => None,
            };
            hints.push((*slot, descriptor, source));
        }
        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
            ordinary: crate::frontend::callables::OrdinaryCallWitness::facade(
                self.frame.body.body().clone(),
                NodeId(site),
                bound.identity(),
            ),
            site,
            hints,
            partial: Vec::new(),
        });
        Ok(())
    }
    pub(super) fn seal_constructor_call(
        &mut self,
        site: u32,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        self.frame
            .seal_constructor_call(site, arguments, &mut self.store)
    }
    /// The Call exit of a sealed constructor call completes in place: the
    /// argument tickets become the declaration-order fields of the bound
    /// variant and the caller resumes with the committed aggregate.
    pub(super) fn complete_constructor_call(
        &mut self,
        action: crate::vm::action::CallAction,
        site: crate::ir::recipe::CallSiteId,
        resume: PcKey,
        result: SlotId,
    ) -> Result<(), Diagnostic> {
        let (bound, args) = self.frame.take_sealed_constructor_call(site.0)?;
        let consumed =
            action.consume(&self.frame.actions, self.frame.pc(), site, &resume, result)?;
        let packet = self
            .frame
            .call_arguments
            .take()
            .ok_or_else(|| call_fault("constructor Call exit has no argument packet"))?;
        let tickets = packet.take_tickets(&mut self.store)?;
        let ticket = construct_from_arguments(&bound, &args, &tickets, &mut self.store)?;
        let ty = crate::runtime::shape::TypeId(self.store.peek_ticket(ticket)?.type_id);
        self.frame
            .resume_call(consumed.resume, consumed.result, ticket, &mut self.store)?;
        self.frame
            .publish_constructor_result(consumed.result, ty, &self.store)
    }
    /// A lowered unqualified callee spelling must never fall back to a
    /// same-named module item when an entered lexical item may bind it.
    pub(super) fn requires_lexical_callee_context(
        &mut self,
        db: &mut Database,
        name: &str,
    ) -> QueryPoll<bool> {
        use crate::db::dependency::{InputKey, InputValue};
        let definition = self.frame.pc().body.definition;
        let source = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
            Some(InputValue::DefinitionSource(source)) => source.clone(),
            _ => return QueryPoll::Ready(Err(call_fault("reached caller source absent"))),
        };
        let handle = match db.input(&InputKey::DefinitionBody(definition)).as_deref() {
            Some(InputValue::DefinitionBody(handle)) if handle.key() == self.frame.pc().body => {
                handle.clone()
            }
            _ => {
                return QueryPoll::Ready(Err(call_fault("reached caller body identity differs")));
            }
        };
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(source)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("reached caller parse kind differs")));
        };
        QueryPoll::Ready(parsed.entered_call_requires_lexical_context(
            &handle,
            &self.frame.body,
            self.frame.pc(),
            name,
        ))
    }
}
