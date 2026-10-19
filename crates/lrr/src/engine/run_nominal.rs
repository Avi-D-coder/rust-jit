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
    calls: BTreeMap<
        (
            BodyKey,
            crate::vm::call::CallIdentity,
            Option<crate::traits::term::TyTerm>,
        ),
        Option<Arc<BoundConstructor>>,
    >,
    /// Whether each call site's callee is bound lexically (a block item or
    /// import), decided once per site.
    lexical_callees: BTreeMap<(BodyKey, crate::vm::call::CallIdentity), bool>,
    /// A `Root::member` callee whose root an import binds to a module,
    /// rewritten to the module route's spelling (`None`: unchanged).
    module_names: BTreeMap<(BodyKey, crate::vm::call::CallIdentity), Option<String>>,
    /// Call sites whose lexical callee is a closed facade function a
    /// block-level import brought into scope (`None`: it is not one).
    lexical_facades: BTreeMap<
        (BodyKey, crate::vm::call::CallIdentity),
        Option<(
            crate::facade::functions::LibraryFunction,
            crate::core::digest::Digest,
        )>,
    >,
    /// Held legacy unnormalized field selections, unchanged until actual
    /// source catalogue publication supplies a retained BodyEnvironment.
    legacy_positional:
        BTreeMap<(PcKey, crate::core::digest::Digest), crate::frontend::types::BoundNominalField>,
    /// Ready record activations retain the same source plan and immutable
    /// specialized mapping once per actual generic application.
    pub(super) records: BTreeMap<
        crate::frontend::nominal_member_goals::NominalMemberKey,
        crate::frontend::prepared_constructors::PreparedRecordConstructorHandle,
    >,
    /// Current pure member specializations; runtime representation is an
    /// additional guard key, never part of the memo request.
    members: BTreeMap<
        (
            crate::frontend::nominal_member_goals::NominalMemberKey,
            Option<crate::runtime::shape::TypeId>,
        ),
        (Arc<QueryValue>, Arc<crate::db::memo::PreparedInputGuard>),
    >,
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
        if matches!(self.graph, Some(RunNames::Legacy(_))) {
            return self.prepare_legacy_positional_field(db, base, field, destination, definition);
        }
        self.prepare_selected_field(
            db,
            base,
            destination,
            definition,
            crate::frontend::nominal_member_goals::NominalMemberSelection::Positional(field),
        )
    }

    fn prepare_legacy_positional_field(
        &mut self,
        db: &mut Database,
        base: SlotId,
        field: u32,
        destination: SlotId,
        definition: crate::core::digest::Digest,
    ) -> QueryPoll<()> {
        let key = (self.frame.pc().clone(), definition);
        if let Some(binding) = self.nominal.legacy_positional.get(&key) {
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
        let owner = match crate::frontend::source_body_context::frame_context(
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
        self.nominal.legacy_positional.insert(key, binding.clone());
        QueryPoll::Ready(self.frame.prepare_source_field(base, destination, binding))
    }
    /// An admitted context is source identity; a runtime descriptor may supply
    /// its actual closed arguments after the declaration has been selected.
    pub(super) fn constructor_context(
        &self,
        site: u32,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        let node = self
            .frame
            .body
            .nodes()
            .get(site as usize)
            .ok_or_else(|| call_fault("constructor context node outside body index"))?;
        // Constructor classification is also probed for valid synthetic
        // operator/pattern sites. They need no expression result context.
        let Some(slot) = node.value else {
            return Ok(None);
        };
        if let Some(source) = self.frame.source_view_ref(slot)? {
            return Ok(Some(self.frame.bound_view(source, &self.store)));
        }
        Ok(self
            .frame
            .hint(slot)?
            .and_then(|ty| self.store.descriptor_source_term(ty)))
    }

    pub(super) fn nominal_member(
        &mut self,
        db: &mut Database,
        key: crate::frontend::nominal_member_goals::NominalMemberKey,
        runtime: Option<crate::runtime::shape::TypeId>,
    ) -> QueryPoll<(Arc<QueryValue>, Arc<crate::db::memo::PreparedInputGuard>)> {
        let cache_key = (key.clone(), runtime);
        if let Some((value, guard)) = self.nominal.members.get(&cache_key) {
            return match guard.require_current(db) {
                Ok(()) => QueryPoll::Ready(Ok((Arc::clone(value), Arc::clone(guard)))),
                Err(error) => QueryPoll::Ready(Err(error)),
            };
        }
        let demand = QueryKey::NominalMember(Box::new(key));
        let fetched = match db.fetch(&SyntaxProvider, demand.clone()) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let guard = match db.prepare_input_guard(&SyntaxProvider, &demand, fetched.fingerprint) {
            QueryPoll::Ready(Ok(guard)) => Arc::new(guard),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let value = fetched.value;
        self.nominal
            .members
            .insert(cache_key, (Arc::clone(&value), Arc::clone(&guard)));
        QueryPoll::Ready(Ok((value, guard)))
    }

    pub(super) fn bind_selected_constructor(
        &self,
        db: &mut Database,
        target: &crate::frontend::nominal_constructors::NominalConstructorTarget,
    ) -> QueryPoll<BoundConstructor> {
        if let Some(RunNames::Legacy(graph)) = &self.graph {
            let sources: Vec<_> = self.sources.iter().collect();
            return bind_constructor(db, graph, &sources, target);
        }
        let context = match self.constructor_context(self.frame.pc().node) {
            Ok(context) => context,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let arguments = match context {
            Some(crate::traits::term::TyTerm::Nominal(definition, arguments))
                if definition == target.identity() =>
            {
                arguments
            }
            _ => Vec::new(),
        };
        if arguments.is_empty() {
            let Some(graph) = &self.graph else {
                return QueryPoll::Ready(Err(call_fault("constructor source graph absent")));
            };
            let sources: Vec<_> = self.sources.iter().collect();
            return bind_constructor(db, graph, &sources, target);
        }
        let key = crate::frontend::nominal_member_goals::NominalMemberKey {
            pc: PcKey {
                phase: 0,
                ..self.frame.pc().clone()
            },
            definition: target.identity(),
            arguments,
            selection: crate::frontend::nominal_member_goals::NominalMemberSelection::Variant(
                target.variant_name().map(str::to_owned),
            ),
        };
        // This helper is used while selecting a cold target; its result and
        // exact guard are retained by the call-site cache. Database memo hits
        // share the same immutable variant across activations.
        let demand = QueryKey::NominalMember(Box::new(key));
        let selected = match db.fetch(&SyntaxProvider, demand.clone()) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::NominalMember(member) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault(
                "constructor specialization outcome differs",
            )));
        };
        let Some(variant) = member.variant() else {
            return QueryPoll::Ready(Err(call_fault("constructor specialization variant absent")));
        };
        let guard = match db.prepare_input_guard(&SyntaxProvider, &demand, selected.fingerprint) {
            QueryPoll::Ready(Ok(guard)) => Arc::new(guard),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(BoundConstructor::specialized(
            Arc::clone(variant),
            member.shape(),
            guard,
        )))
    }

    pub(super) fn prepare_selected_field(
        &mut self,
        db: &mut Database,
        base: SlotId,
        destination: SlotId,
        definition: crate::core::digest::Digest,
        selection: crate::frontend::nominal_member_goals::NominalMemberSelection,
    ) -> QueryPoll<()> {
        let source = match self.frame.source_view_ref(base) {
            Ok(Some(source)) => self.frame.bound_view(source, &self.store),
            Ok(None) => {
                return QueryPoll::Ready(Err(call_fault("nominal field source arguments absent")));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let source = match &source {
            crate::traits::term::TyTerm::Ref(_, _, target) => target.as_ref(),
            _ => &source,
        };
        let crate::traits::term::TyTerm::Nominal(owner, args) = source else {
            return QueryPoll::Ready(Err(call_fault("nominal field source application absent")));
        };
        if *owner != definition {
            return QueryPoll::Ready(Err(call_fault("nominal field source declaration differs")));
        }
        let arguments = args.clone();
        let runtime = match self.frame.place_type(base, &self.store) {
            Ok(ty) => ty,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let key = crate::frontend::nominal_member_goals::NominalMemberKey {
            pc: PcKey {
                phase: 0,
                ..self.frame.pc().clone()
            },
            definition,
            arguments,
            selection,
        };
        let (selected, _guard) = match self.nominal_member(db, key, Some(runtime)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::NominalMember(member) = selected.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("field specialization outcome differs")));
        };
        let Some(binding) = member.field() else {
            return QueryPoll::Ready(Err(call_fault("field specialization binding absent")));
        };
        QueryPoll::Ready(
            self.frame
                .prepare_source_field(base, destination, binding.clone()),
        )
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
        self.bind_selected_constructor(db, &target).map(Some)
    }
    /// Install the unit constructor selected for the `ValuePath` at this PC.
    pub(super) fn prepare_value_path(
        &mut self,
        db: &mut Database,
        path: &str,
        stated: Option<&crate::ir::written_type::WrittenType>,
    ) -> QueryPoll<()> {
        let explicit = match self.constructor_arguments(db) {
            QueryPoll::Ready(Ok(Some(arguments))) => {
                match arguments.value_descriptors(&self.frame, &mut self.store) {
                    Ok(arguments) => Some(arguments),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            QueryPoll::Ready(Ok(None)) => None,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let pc = self.frame.pc().clone();
        if self.frame.has_cached_item_value() {
            return self.prepare_item_value_path(db, path);
        }
        // A genuine source Type parameter shadows module/constructor names.
        // Only the retained hygienic qualifier proof can establish that fact.
        match self.prepare_parameter_associated_value(db, path) {
            QueryPoll::Ready(Ok(true)) => return QueryPoll::Ready(Ok(())),
            QueryPoll::Ready(Ok(false)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        // A static read by value: its selection is cached per PC; only the
        // cell's current contents are read on each execution.
        if let Some((cell, view)) = self.static_reads.get(&pc).cloned() {
            return QueryPoll::Ready(self.read_static_value(cell, view));
        }
        // A constructor already selected for this PC (lexically or through
        // the module graph) is only installed on each execution.
        if let Some(bound) = self.nominal.values.get(&pc) {
            return QueryPoll::Ready(self.frame.prepare_nominal_value(
                Arc::clone(bound.variant()),
                stated,
                explicit.as_deref(),
                &mut self.store,
            ));
        }
        // Lexical (body-local) scope precedes the module graph.
        match self.prepare_lexical_value_path(db, path, stated, explicit.as_deref()) {
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
                QueryPoll::Ready(Ok(None)) if explicit.is_some() => {
                    return QueryPoll::Ready(Err(unsupported(
                        "explicit Type arguments on a non-constructor value require their value contract integration",
                    )));
                }
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
            explicit.as_deref(),
            &mut self.store,
        ))
    }
    /// A lexically selected constructor read as a value: only a unit
    /// constructor is a value (a tuple constructor is a function value).
    pub(super) fn prepare_lexical_constructor_value(
        &mut self,
        bound: BoundConstructor,
        stated: Option<&crate::ir::written_type::WrittenType>,
        explicit: Option<&[Option<crate::runtime::shape::TypeId>]>,
    ) -> QueryPoll<()> {
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
                    "record struct or variant is not a value; use its record constructor".into(),
                )));
            }
        }
        let pc = self.frame.pc().clone();
        let bound = Arc::new(bound);
        self.nominal.values.insert(pc, Arc::clone(&bound));
        QueryPoll::Ready(self.frame.prepare_nominal_value(
            Arc::clone(bound.variant()),
            stated,
            explicit,
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
        // Method calls (`.name`) are selected from their receiver's type.
        if self.pending.contains_key(&identity) || name.starts_with('.') {
            return QueryPoll::Ready(Ok(false));
        }
        let context = match self.constructor_context(site) {
            Ok(context) => context,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let key = (self.frame.pc().body.clone(), identity, context);
        // The classification of a call site is fixed for the run's source
        // publication; ordinary calls are probed once, not per execution.
        let bound = if let Some(cached) = self.nominal.calls.get(&key) {
            match cached {
                Some(bound) => Arc::clone(bound),
                None => return QueryPoll::Ready(Ok(false)),
            }
        } else {
            // A callee a block item or import may bind is selected by its
            // entered lexical Value goal, never by the module graph.
            let lexical = match self.requires_lexical_callee_context(db, name) {
                QueryPoll::Ready(Ok(lexical)) => lexical,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let selected = if lexical {
                match self.lexical_call_constructor(db, site) {
                    QueryPoll::Ready(Ok(selected)) => selected,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            } else {
                match self.select_nominal_constructor_with(db, name, true) {
                    QueryPoll::Ready(Ok(selected)) => selected,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
            let bound = match selected {
                Some(bound) => bound,
                None => {
                    self.nominal.calls.insert(key, None);
                    return QueryPoll::Ready(Ok(false));
                }
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
        if let Err(error) = bound.require_current(db) {
            return QueryPoll::Ready(Err(error));
        }
        let explicit = match self.constructor_arguments(db) {
            QueryPoll::Ready(Ok(Some(arguments))) => {
                match arguments.value_descriptors(&self.frame, &mut self.store) {
                    Ok(arguments) => Some(Arc::from(arguments)),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            QueryPoll::Ready(Ok(None)) => None,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.frame
            .select_constructor_call(site, Arc::clone(bound.variant()), explicit);
        QueryPoll::Ready(Ok(true))
    }
    /// The written callee of a `Root::member` call whose root a block-level
    /// import binds to a module (`use std::mem; mem::swap(..)`), spelled from
    /// the module's defining crate so the module route resolves it; any other
    /// name is returned unchanged. Decided once per call site.
    pub(super) fn lexical_module_name(
        &mut self,
        db: &mut Database,
        site: u32,
        name: String,
    ) -> QueryPoll<String> {
        if !name.contains("::") || name.starts_with(['$', '<', ':']) {
            return QueryPoll::Ready(Ok(name));
        }
        let key = (self.frame.pc().body.clone(), self.frame.call_identity(site));
        if let Some(known) = self.nominal.module_names.get(&key) {
            return QueryPoll::Ready(Ok(known.clone().unwrap_or(name)));
        }
        let lexical = match self.requires_lexical_callee_context(db, &name) {
            QueryPoll::Ready(Ok(lexical)) => lexical,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut rewritten = None;
        if lexical
            && let Some(path) = self
                .frame
                .body
                .nodes()
                .get(site as usize)
                .and_then(|call| call.operands.first().copied())
        {
            let goal = PcKey {
                body: self.frame.pc().body.clone(),
                node: path.0,
                phase: 0,
            };
            match self.entered_value_declaration(db, goal) {
                QueryPoll::Ready(Ok(Some(selected))) => {
                    if let (Some(root), Some(member)) = (&selected.module_root, &selected.rooted) {
                        rewritten = Some(format!("{root}::{member}"));
                    }
                }
                QueryPoll::Ready(Ok(None)) | QueryPoll::Ready(Err(_)) => {}
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        if rewritten.is_some() {
            // The module route now owns this site: it is no longer lexical.
            self.nominal.lexical_callees.insert(key.clone(), false);
        }
        self.nominal.module_names.insert(key, rewritten.clone());
        QueryPoll::Ready(Ok(rewritten.unwrap_or(name)))
    }
    /// The closed facade function the entered lexical Value goal of this
    /// call's callee selects (`use std::cmp::max; max(a, b)`), if any. Cold
    /// once per call site; later executions find the cached answer.
    pub(super) fn lexical_facade_call(
        &mut self,
        db: &mut Database,
        site: u32,
        name: &str,
    ) -> QueryPoll<
        Option<(
            crate::facade::functions::LibraryFunction,
            crate::core::digest::Digest,
        )>,
    > {
        if name.contains("::") || name.starts_with('.') {
            return QueryPoll::Ready(Ok(None));
        }
        let key = (self.frame.pc().body.clone(), self.frame.call_identity(site));
        if let Some(cached) = self.nominal.lexical_facades.get(&key) {
            return QueryPoll::Ready(Ok(*cached));
        }
        let lexical = match self.requires_lexical_callee_context(db, name) {
            QueryPoll::Ready(Ok(lexical)) => lexical,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut selected = None;
        if lexical
            && let Some(path) = self
                .frame
                .body
                .nodes()
                .get(site as usize)
                .and_then(|call| call.operands.first().copied())
        {
            let goal = PcKey {
                body: self.frame.pc().body.clone(),
                node: path.0,
                phase: 0,
            };
            match self.entered_value_declaration(db, goal) {
                QueryPoll::Ready(Ok(found)) => selected = found.and_then(|found| found.facade),
                // An unsettled goal leaves the callee to the source-call
                // route, which reports its own missing evidence.
                QueryPoll::Ready(Err(_)) => {}
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        self.nominal.lexical_facades.insert(key, selected);
        QueryPoll::Ready(Ok(selected))
    }
    /// The constructor the entered lexical Value goal of this call's callee
    /// path selects (a block-local struct, or a struct or variant a
    /// block-level import brings into scope). `None`: the callee is some
    /// other item or a binding, selected by the ordinary source-call route.
    fn lexical_call_constructor(
        &mut self,
        db: &mut Database,
        site: u32,
    ) -> QueryPoll<Option<BoundConstructor>> {
        let Some(path) = self
            .frame
            .body
            .nodes()
            .get(site as usize)
            .and_then(|call| call.operands.first().copied())
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let key = PcKey {
            body: self.frame.pc().body.clone(),
            node: path.0,
            phase: 0,
        };
        let selected = match self.entered_value_declaration(db, key) {
            QueryPoll::Ready(Ok(Some(selected))) => selected,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            // A goal that cannot be settled leaves the callee to the source
            // call route, which reports its own missing evidence.
            QueryPoll::Ready(Err(_)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.lexical_constructor(db, &selected)
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
        let seeded = self.frame.constructor_type_arguments(site, &mut self.store)?;
        let mut hints = Vec::new();
        hints
            .try_reserve_exact(arguments.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for (slot, field) in arguments.iter().zip(bound.fields()) {
            // A field naming one of the declaration's parameters is hinted
            // only once the call's context fixes the type arguments.
            let descriptor = field.materialize_partial(&mut self.store, &seeded)?;
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
        // The answer is a fact of the call site's source and goal: cold once.
        let site = (
            self.frame.pc().body.clone(),
            self.frame.call_identity(self.frame.pc().node),
        );
        if let Some(known) = self.nominal.lexical_callees.get(&site) {
            return QueryPoll::Ready(Ok(*known));
        }
        match self.lexical_callee_context(db, name) {
            QueryPoll::Ready(Ok(lexical)) => {
                self.nominal.lexical_callees.insert(site, lexical);
                QueryPoll::Ready(Ok(lexical))
            }
            other => other,
        }
    }
    fn lexical_callee_context(&mut self, db: &mut Database, name: &str) -> QueryPoll<bool> {
        use crate::db::dependency::{InputKey, InputValue};
        // Only `Root::member` can be lexical among qualified names: longer,
        // anchored, `$crate`, `<T as Trait>` and `Self` paths are the module
        // route's and need no caller source.
        let qualified = name.contains("::");
        if qualified {
            let mut parts = name.split("::");
            let (Some(root), Some(member), None) = (parts.next(), parts.next(), parts.next())
            else {
                return QueryPoll::Ready(Ok(false));
            };
            if root.is_empty()
                || member.is_empty()
                || root.starts_with(['$', '<'])
                || matches!(root, "crate" | "self" | "super" | "Self")
            {
                return QueryPoll::Ready(Ok(false));
            }
        }
        let definition = self.frame.pc().body.definition;
        let source = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
            Some(InputValue::DefinitionSource(source)) => source.clone(),
            _ if qualified => return QueryPoll::Ready(Ok(false)),
            _ => return QueryPoll::Ready(Err(call_fault("reached caller source absent"))),
        };
        let handle = match db.input(&InputKey::DefinitionBody(definition)).as_deref() {
            Some(InputValue::DefinitionBody(handle)) if handle.key() == self.frame.pc().body => {
                handle.clone()
            }
            _ if qualified => return QueryPoll::Ready(Ok(false)),
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
        let syntactic = match parsed.entered_call_requires_lexical_context(
            &handle,
            &self.frame.body,
            self.frame.pc(),
            name,
        ) {
            Ok(syntactic) => syntactic,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if !syntactic || !name.contains("::") {
            return QueryPoll::Ready(Ok(syntactic));
        }
        // `Root::member`: a block item or import may bind the root, but the
        // callee is lexical only when the entered Value goal actually bound
        // it; otherwise the module route selects the written path.
        let Some(path) = self
            .frame
            .body
            .nodes()
            .get(self.frame.pc().node as usize)
            .and_then(|call| call.operands.first().copied())
        else {
            return QueryPoll::Ready(Ok(false));
        };
        let key = PcKey {
            body: self.frame.pc().body.clone(),
            node: path.0,
            phase: 0,
        };
        let goal = match db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key)) {
            QueryPoll::Ready(Ok(goal)) => goal,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::EnteredValue(goal) = goal.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("lexical callee goal variant differs")));
        };
        QueryPoll::Ready(Ok(goal.rooted().is_some()))
    }
    /// `Root::function(..)` where the entered Value goal bound `Root`
    /// lexically (a block-local type, or an imported one): the associated
    /// function the member index selects for that type, exactly as a module
    /// type's `Type::function` selects it.
    pub(super) fn select_lexical_associated_call(
        &mut self,
        db: &mut Database,
        name: &str,
        syntax: &[String],
        arguments: &[SlotId],
    ) -> QueryPoll<Option<SelectedArgumentCall>> {
        let Some(path) = self
            .frame
            .body
            .nodes()
            .get(self.frame.pc().node as usize)
            .and_then(|call| call.operands.first().copied())
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let key = PcKey {
            body: self.frame.pc().body.clone(),
            node: path.0,
            phase: 0,
        };
        let goal = match db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key)) {
            QueryPoll::Ready(Ok(goal)) => goal,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::EnteredValue(goal) = goal.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("lexical callee goal variant differs")));
        };
        let (Some(member), Some(declaration)) = (goal.rooted(), goal.declaration()) else {
            return QueryPoll::Ready(Ok(None));
        };
        if declaration.variant().is_some() || name.split("::").count() != 2 {
            return QueryPoll::Ready(Ok(None));
        }
        let root = declaration.identity();
        let (Some(members), Some(graph)) = (self.members.clone(), &self.graph) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let selection = match CallResolver::new(graph, &sources) {
            Ok(resolver) => {
                resolver
                    .with_members(Some(&members))
                    .associated_of(db, &members, Ok(root), member)
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match selection {
            QueryPoll::Ready(Ok(Some(CallSelection::Source(callee)))) => self
                .prepare_selected_argument_call(db, Arc::from(callee), syntax, arguments)
                .map(Some),
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
