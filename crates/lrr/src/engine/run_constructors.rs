//! Reached constructor facts are resolved before opcode fuel or guard allocation.
use super::*;
use crate::{
    frontend::constructors::RecordConstructorBinding, syntax::constructors::ConstructorKind,
};

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
impl RunMachine {
    pub(super) fn prepare_installed_constructor(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
        contexts: &[super::super::runtime_macros::RuntimeMacroContext<'_>],
    ) -> QueryPoll<()> {
        match self.installed.prepared_constructor(db, id) {
            Ok(Some(source)) => QueryPoll::Ready(
                self.frame
                    .prepare_installed_nominal_constructor(source, &mut self.store),
            ),
            Ok(None) => self.prepare_constructor(db, contexts),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    // Unsealed legacy/structural entries retain their existing route until
    // their common immutable preparation is delivered. Prepared record entries
    // above never re-enter this query/binder path.
    pub(super) fn prepare_constructor(
        &mut self,
        db: &mut Database,
        contexts: &[super::super::runtime_macros::RuntimeMacroContext<'_>],
    ) -> QueryPoll<()> {
        let outcome = match db.fetch(
            &SyntaxProvider,
            QueryKey::ConstructorPlan(self.frame.pc().clone()),
        ) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let QueryValue::ConstructorPlan(plan) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("constructor plan outcome differs")));
        };
        match plan.kind() {
            ConstructorKind::Record { rest: None, .. }
            | ConstructorKind::Tuple(_)
            | ConstructorKind::Array(_) => {}
            ConstructorKind::Record { rest: Some(_), .. } => {
                return QueryPoll::Ready(Err(unsupported(
                    "record update requires checked base-field transfers",
                )));
            }
            ConstructorKind::Repeat { .. } => {
                return QueryPoll::Ready(Err(unsupported(
                    "array repeat requires reached const length and Copy binding",
                )));
            }
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "constructor requires a checked source catalogue",
            )));
        };
        // A genuine prepared record entry must have installed its binding
        // atomically with the recipe. Never reconstruct a missing payload or
        // fall back after a stale/invalid prepared environment.
        if matches!(plan.kind(), ConstructorKind::Record { rest: None, .. })
            && db
                .published_input(&crate::db::dependency::InputKey::BodyEnvironment(
                    self.frame.body.body().clone(),
                ))
                .is_some()
        {
            return QueryPoll::Ready(Err(call_fault(
                "prepared record recipe lacks installed source binding",
            )));
        }
        // This fixed-size identity has no owned syntax or allocation. Retain
        // it independently while the checked closure prepares frame facts.
        let body = self.frame.body.body().clone();
        let mut owners = self.sources.iter().filter(|view| {
            view.outline()
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .any(|handle| handle.key() == body)
        });
        let Some(source) = owners.next() else {
            return QueryPoll::Ready(Err(call_fault("constructor source owner absent")));
        };
        if owners.next().is_some() {
            return QueryPoll::Ready(Err(call_fault("constructor source owner is duplicated")));
        }
        if matches!(
            plan.kind(),
            ConstructorKind::Tuple(_) | ConstructorKind::Array(_)
        ) {
            return QueryPoll::Ready((|| {
                use crate::db::dependency::{InputKey, InputValue};
                graph.require_source(source.selection(), source.outline())?;
                source.require_constructor_plan(&self.frame.body, plan)?;
                let caller = body.definition;
                let declaration = source
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.fingerprint() == caller)
                    .ok_or_else(|| call_fault("constructor caller declaration absent"))?;
                if !matches!(db.input(&InputKey::DefinitionSource(caller)).as_deref(),
                    Some(InputValue::DefinitionSource(key)) if key == &source.selection().outline.parse)
                    || !matches!(db.input(&InputKey::DefinitionBody(caller)).as_deref(),
                        Some(InputValue::DefinitionBody(handle)) if handle.key() == body)
                    || !matches!(db.input(&InputKey::DefinitionHeader(caller)).as_deref(),
                        Some(InputValue::DefinitionHeader(header)) if header == &declaration.header)
                    || !matches!(db.input(&InputKey::DeclarationContext(caller)).as_deref(),
                        Some(InputValue::DeclarationContext(context)) if context.scope == declaration.scope && context.range == declaration.declaration_span)
                    || !matches!(db.input(&InputKey::CrateCfg(source.selection().outline.crate_key)).as_deref(),
                        Some(InputValue::CrateCfg(cfg)) if cfg.fingerprint() == source.selection().outline.cfg)
                {
                    return Err(call_fault(
                        "constructor caller source revision differs from database",
                    ));
                }
                let result = self.frame.body.nodes()[plan.node().0 as usize]
                    .value
                    .ok_or_else(|| call_fault("structural constructor result slot absent"))?;
                let ty = match self.frame.hint(result)? {
                    Some(ty) => ty,
                    None if matches!(plan.kind(), ConstructorKind::Tuple(fields) if fields.is_empty()) => {
                        crate::runtime::shape::TypeId(1)
                    }
                    None => {
                        // Bind only exact source cardinality now. Reached child
                        // descriptors arrive with completed fields; a diverging
                        // operand need not invent types for fields it never enters.
                        return self.frame.prepare_observed_structural_constructor(plan);
                    }
                };
                // Runtime shape alone cannot erase a retained rigid source
                // requirement. Composite source-view binding is still pending.
                if let Some(written) = self.frame.source_view(result)? {
                    let matches = match written {
                        crate::traits::term::TyTerm::Primitive(primitive) => {
                            crate::frontend::call_contracts::primitive_runtime_id(primitive)? == ty
                        }
                        crate::traits::term::TyTerm::Array(
                            element,
                            crate::traits::term::ConstTerm::Value(len),
                        ) => {
                            if let crate::traits::term::TyTerm::Primitive(primitive) =
                                element.as_ref()
                            {
                                let expected =
                                    crate::frontend::call_contracts::primitive_runtime_id(
                                        *primitive,
                                    )?;
                                matches!(self.store.type_descriptor(ty)?.key.as_ref(), crate::runtime::shape::TypeKey::Array { element, len: actual_len }
                                    if *element == expected && u64::from(*actual_len) == len)
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    if !matches {
                        return Err(Diagnostic::new(
                            "LRR-E-GENERIC-BINDING",
                            "structural constructor result requires its source type binding".into(),
                        ));
                    }
                }
                self.frame
                    .prepare_structural_constructor(plan, ty, &self.store)
            })());
        }
        let mut sources = Vec::new();
        if sources.try_reserve_exact(self.sources.len()).is_err() {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        sources.extend(self.sources.iter());
        let binding = if let Some(context) = contexts
            .iter()
            .find(|context| context.source().selection() == source.selection())
        {
            context.bind_record_constructor(
                db,
                graph,
                &sources,
                &self.frame.body,
                plan,
                self.runtime_no_item_proofs.values(),
            )
        } else {
            // Original source uses the pinned root-context adapter. Never give
            // generated tokens fabricated root hygiene or drop producer ribs.
            let index = (|| {
                source.require_original_hygiene()?;
                let root = crate::expand::source_hygiene::source_root(
                    source.selection().outline.crate_key,
                    source.selection().outline.parse.profile.edition,
                )?;
                let facts = crate::frontend::hygiene::HygieneFacts::new(root.contexts, root.rows)?
                    .with_name_contexts(root.names)?;
                crate::frontend::source_macros::PublishedMacroIndex::from_view(
                    source.share(),
                    root.context,
                    facts,
                )
            })();
            let index = match index {
                Ok(index) => index,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let entered = match index.enter_body(source.selection(), &body) {
                Ok(entered) => entered,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            crate::frontend::constructors::bind_record_constructor(
                db,
                graph,
                &sources,
                source,
                &entered,
                &self.frame.body,
                plan,
            )
        };
        match binding {
            QueryPoll::Ready(Ok(RecordConstructorBinding::Ready(bound))) => QueryPoll::Ready(
                self.frame
                    .prepare_nominal_constructor(plan, *bound, &mut self.store),
            ),
            QueryPoll::Ready(Ok(RecordConstructorBinding::Deferred(demand))) => {
                QueryPoll::Ready(Err(demand.diagnostic()))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        runtime::shape::{TypeId, TypeKey},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    const LARGE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/large-records.rs"
    );
    const UNARY: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/one-tuple.rs");
    const ARRAY: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/vec-fixed-length.rs"
    );
    const UNIT: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/cast/issue-10991.rs");
    const EMPTY_ARRAY: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/bool-comparison-in-const-array-len.rs"
    );
    const NESTED_TUPLE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/index-float.rs");
    fn stock_structural(source: &str) -> (Database, RunMachine, SlotId) {
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("structural.rs").unwrap();
        let unit = Digest::of_bytes(b"rustc-structural-dispatch");
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("stock structural source absent")
        };
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let mut run = RunMachine::start_with_sources(&mut db, body, 100, vec![view.share()])
            .into_ready()
            .unwrap()
            .unwrap();
        let node = run
            .frame
            .body
            .nodes()
            .iter()
            .rfind(|node| node.constructor_guard.is_some())
            .unwrap()
            .clone();
        let mut scopes = vec![];
        let mut scope = Some(node.scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = run.frame.body.scopes()[id.0 as usize].parent;
        }
        for scope in scopes.into_iter().rev() {
            run.frame.enter(scope).unwrap();
        }
        run.frame.pc = run.frame.body.pc(node.id, Phase::Entry).unwrap();
        (db, run, node.value.unwrap())
    }
    /// Execute only the actual source constructor's reached thunks. The caller
    /// supplies an already-bound worker descriptor; no type inference or later
    /// let, pattern, indexing, assertion or cast is simulated by this harness.
    fn execute_structural_prefix(db: &mut Database, run: &mut RunMachine, fields: usize) {
        let handles = run.store.budget().handles();
        let fuel = run.frame.fuel.snapshot().used;
        run.prepare_constructor(db, &[])
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.frame.fuel.snapshot().used, fuel);
        assert_eq!(run.frame.counters.operations_started, 0);
        for _ in 0..(2 * fields + 2) {
            let result = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = result.value.as_ref() else {
                panic!("actual constructor thunk absent")
            };
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        assert_eq!(
            run.frame.counters.operations_completed,
            (2 * fields + 2) as u64
        );
    }
    #[test]
    fn rustc_unary_tuple_dispatch_uses_bound_descriptor_and_preserves_original_field() {
        let (mut db, mut run, destination) = stock_structural(UNARY);
        let tuple = run
            .store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        // An explicit worker context, not inferred from the local type spelling.
        run.frame.set_hint(destination, Some(tuple)).unwrap();
        execute_structural_prefix(&mut db, &mut run, 1);
        let ticket = run
            .store
            .move_from(run.frame.cell(destination).unwrap())
            .unwrap();
        let field = run.store.aggregate_field(ticket, 0).unwrap();
        let value = run.store.read_scalar(field).unwrap();
        assert_eq!(value.type_id, 3);
        assert_eq!(value.payload, u64::from('d'));
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(event.key, QueryKey::TypeSchema(_) | QueryKey::Constant(_)))
        );
        let field = run.store.aggregate_cleanup_next(ticket).unwrap().unwrap();
        run.store.discard(field).unwrap();
        assert!(run.store.aggregate_cleanup_next(ticket).unwrap().is_none());
        run.store.discard(ticket).unwrap();
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_array_dispatch_uses_bound_isize_and_original_written_order() {
        let (mut db, mut run, destination) = stock_structural(ARRAY);
        let array = run
            .store
            .intern_type(TypeKey::Array {
                element: TypeId(8),
                len: 4,
            })
            .unwrap();
        run.frame.set_hint(destination, Some(array)).unwrap();
        execute_structural_prefix(&mut db, &mut run, 4);
        let ticket = run
            .store
            .move_from(run.frame.cell(destination).unwrap())
            .unwrap();
        for (field, expected) in [1, 2, 3, 4].into_iter().enumerate() {
            let field = run.store.aggregate_field(ticket, field as u32).unwrap();
            let value = run.store.read_scalar(field).unwrap();
            assert_eq!(value.type_id, 8);
            assert_eq!(value.payload, expected);
        }
        for _ in 0..4 {
            let field = run.store.aggregate_cleanup_next(ticket).unwrap().unwrap();
            run.store.discard(field).unwrap();
        }
        assert!(run.store.aggregate_cleanup_next(ticket).unwrap().is_none());
        run.store.discard(ticket).unwrap();
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_unit_tuple_dispatch_requires_no_invented_element_context() {
        let (mut db, mut run, destination) = stock_structural(UNIT);
        assert_eq!(run.frame.hint(destination).unwrap(), None);
        execute_structural_prefix(&mut db, &mut run, 0);
        let value = run
            .store
            .read_scalar(run.frame.cell(destination).unwrap())
            .unwrap();
        assert_eq!(value, ValueRepr::unit());
        // The original invalid unit-to-usize cast is not entered or accepted.
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_empty_array_dispatch_requires_element_context_even_without_fields() {
        let (mut db, mut run, destination) = stock_structural(EMPTY_ARRAY);
        let handles = run.store.budget().handles();
        let missing = run
            .prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(missing.code, "LRR-E-TYPE-CONTEXT");
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.frame.fuel.snapshot().used, 0);
        assert!(run.frame.cell(destination).is_err());
        // Explicit bound worker context only. The source alias's comparison
        // and const-length cast are not evaluated by this constructor control.
        let array = run
            .store
            .intern_type(TypeKey::Array {
                element: TypeId(1),
                len: 0,
            })
            .unwrap();
        run.frame.set_hint(destination, Some(array)).unwrap();
        execute_structural_prefix(&mut db, &mut run, 0);
        let ticket = run
            .store
            .move_from(run.frame.cell(destination).unwrap())
            .unwrap();
        let value = run.store.peek_ticket(ticket).unwrap();
        assert_eq!(value.kind, 8);
        assert_eq!(value.type_id, array.0);
        assert!(matches!(
            run.store.type_descriptor(array).unwrap().key.as_ref(),
            TypeKey::Array { element, len: 0 } if *element == TypeId(1)
        ));
        assert!(run.store.aggregate_field(ticket, 0).is_err());
        assert!(run.store.aggregate_cleanup_next(ticket).unwrap().is_none());
        run.store.discard(ticket).unwrap();
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_nested_tuple_dispatch_propagates_bound_child_context_before_each_guard() {
        let (mut db, mut run, _) = stock_structural(NESTED_TUPLE);
        // This source has one outer (((),),) constructor. The innermost tuple
        // selected by the common helper shares its captured statement scope.
        let outer = run
            .frame
            .body
            .nodes()
            .iter()
            .find(|node| node.constructor_guard.is_some())
            .unwrap()
            .clone();
        assert!(run.frame.active_scopes().contains(&outer.scope));
        run.frame.pc = run.frame.body.pc(outer.id, Phase::Entry).unwrap();
        let destination = outer.value.unwrap();
        let inner = run
            .store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(1)])))
            .unwrap();
        let outer_type = run
            .store
            .intern_type(TypeKey::Tuple(Box::new([inner])))
            .unwrap();
        run.frame.set_hint(destination, Some(outer_type)).unwrap();
        let mut entries = 0;
        for _ in 0..8 {
            let outcome = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("original nested tuple thunk absent")
            };
            if recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::ConstructStart { .. }))
            {
                let handles = run.store.budget().handles();
                let fuel = run.frame.fuel.snapshot().used;
                run.prepare_constructor(&mut db, &[])
                    .into_ready()
                    .unwrap()
                    .unwrap();
                assert_eq!(run.store.budget().handles(), handles);
                assert_eq!(run.frame.fuel.snapshot().used, fuel);
                entries += 1;
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        assert_eq!(entries, 3);
        assert_eq!(run.frame.counters.operations_completed, 8);
        assert_eq!(run.frame.counters.started_by_opcode[27], 3);
        assert_eq!(run.frame.counters.started_by_opcode[28], 2);
        assert_eq!(run.frame.counters.started_by_opcode[29], 3);
        let outer_ticket = run
            .store
            .move_from(run.frame.cell(destination).unwrap())
            .unwrap();
        assert_eq!(
            run.store.peek_ticket(outer_ticket).unwrap().type_id,
            outer_type.0
        );
        // Harness-owned storage inspection/cleanup only. The original source's
        // commented/whitespace tuple projections and patterns are unexecuted.
        let inner_ticket = run
            .store
            .aggregate_cleanup_next(outer_ticket)
            .unwrap()
            .unwrap();
        assert_eq!(
            run.store.peek_ticket(inner_ticket).unwrap().type_id,
            inner.0
        );
        let unit_cell = run.store.aggregate_field(inner_ticket, 0).unwrap();
        assert_eq!(run.store.read_scalar(unit_cell).unwrap(), ValueRepr::unit());
        let unit_ticket = run
            .store
            .aggregate_cleanup_next(inner_ticket)
            .unwrap()
            .unwrap();
        run.store.discard(unit_ticket).unwrap();
        assert!(
            run.store
                .aggregate_cleanup_next(inner_ticket)
                .unwrap()
                .is_none()
        );
        run.store.discard(inner_ticket).unwrap();
        assert!(
            run.store
                .aggregate_cleanup_next(outer_ticket)
                .unwrap()
                .is_none()
        );
        run.store.discard(outer_ticket).unwrap();
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_structural_dispatch_observes_absent_context_but_refuses_wrong_source_before_effects() {
        let (mut db, mut run, destination) = stock_structural(UNARY);
        let handles = run.store.budget().handles();
        let before = run.frame.fuel.snapshot().used;
        let types = run.store.type_metadata_bytes();
        run.prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.store.type_metadata_bytes(), types);
        assert_eq!(run.frame.fuel.snapshot().used, before);
        assert!(run.frame.cell(destination).is_err());
        // Successful preparation retains exact dispatcher facts. A second
        // preparation cannot silently replace them, even before guard entry.
        let duplicate_prepare = run
            .prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(duplicate_prepare.code, "LRR-E-INTERNAL-IR");
        assert_eq!(
            duplicate_prepare.message,
            "constructor preparation requires the exact entered source plan"
        );
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.store.type_metadata_bytes(), types);
        assert_eq!(run.frame.fuel.snapshot().used, before);
        db.finish_run().unwrap();

        // The wrong-context cases have their own unchanged-source frontier;
        // they must reach descriptor/source admission, not pending-fact refusal.
        let (mut db, mut run, destination) = stock_structural(UNARY);
        let handles = run.store.budget().handles();
        let before = run.frame.fuel.snapshot().used;
        run.frame.set_hint(destination, Some(TypeId(3))).unwrap();
        let wrong = run
            .prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(wrong.code, "LRR-E-INTERNAL-IR");
        assert_eq!(
            wrong.message,
            "structural constructor plan/descriptor shape mismatch"
        );
        let tuple = run
            .store
            .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
            .unwrap();
        run.frame.set_hint(destination, Some(tuple)).unwrap();
        run.frame
            .set_source_view(
                destination,
                Some(crate::traits::term::TyTerm::Param(
                    crate::traits::term::ParamKey {
                        definition: run.frame.body.body().definition,
                        ordinal: 0,
                    },
                )),
            )
            .unwrap();
        let rigid = run
            .prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(rigid.code, "LRR-E-GENERIC-BINDING");
        assert!(matches!(
            run.frame.source_view(destination).unwrap(),
            Some(crate::traits::term::TyTerm::Param(_))
        ));
        run.frame.set_source_view(destination, None).unwrap();
        run.frame.set_hint(destination, None).unwrap();
        run.sources.push(run.sources[0].share());
        let duplicate = run
            .prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(duplicate.message, "constructor source owner is duplicated");
        run.sources.clear();
        let owner = run
            .prepare_constructor(&mut db, &[])
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(owner.message, "constructor source owner absent");
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.frame.fuel.snapshot().used, before);
        assert_eq!(run.frame.counters.operations_started, 0);
        assert!(run.frame.cell(destination).is_err());
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_structural_dispatch_rejects_stale_caller_metadata_before_effects() {
        use crate::db::dependency::{InputKey, InputValue};
        // Retain the original one-tuple source and body while publishing a
        // divergent caller input between runs. No guest opcode has started.
        for changed in ["header", "scope", "range", "cfg", "body"] {
            let (mut db, mut run, destination) = stock_structural(UNARY);
            let tuple = run
                .store
                .intern_type(TypeKey::Tuple(Box::new([TypeId(3)])))
                .unwrap();
            run.frame.set_hint(destination, Some(tuple)).unwrap();
            let caller = run.frame.body.body().definition;
            let mut inputs = run.sources[0].prepared_inputs().clone();
            match changed {
                "header" => {
                    let InputValue::DefinitionHeader(header) =
                        inputs.get_mut(&InputKey::DefinitionHeader(caller)).unwrap()
                    else {
                        panic!("original caller header absent")
                    };
                    assert_eq!(
                        header.visibility,
                        crate::syntax::outline::Visibility::Public
                    );
                    header.visibility = crate::syntax::outline::Visibility::Private;
                }
                "scope" | "range" => {
                    let InputValue::DeclarationContext(context) = inputs
                        .get_mut(&InputKey::DeclarationContext(caller))
                        .unwrap()
                    else {
                        panic!("original caller context absent")
                    };
                    if changed == "scope" {
                        context.scope.owner = Some(Digest::of_bytes(b"foreign-owner"));
                    } else {
                        context.range.end = context.range.end.checked_sub(1).unwrap();
                    }
                }
                "cfg" => {
                    let unit = run.sources[0].selection().outline.crate_key;
                    let InputValue::CrateCfg(cfg) =
                        inputs.get_mut(&InputKey::CrateCfg(unit)).unwrap()
                    else {
                        panic!("original caller cfg absent")
                    };
                    assert!(cfg.atoms.insert("lrr_stale_constructor".into()));
                }
                "body" => {
                    let InputValue::DefinitionBody(body) =
                        inputs.get_mut(&InputKey::DefinitionBody(caller)).unwrap()
                    else {
                        panic!("original caller body absent")
                    };
                    let stale = Digest::of_bytes(b"foreign-constructor-body");
                    assert_ne!(body.body_hash, stale);
                    body.body_hash = stale;
                }
                _ => unreachable!(),
            }
            db.finish_run().unwrap();
            db.publish_inputs(inputs).unwrap();
            db.begin_run().unwrap();
            let handles = run.store.budget().handles();
            let fuel = run.frame.fuel.snapshot().used;
            let pc = run.frame.pc().clone();
            let error = run
                .prepare_constructor(&mut db, &[])
                .into_ready()
                .unwrap()
                .unwrap_err();
            let (code, message) = match changed {
                "cfg" => ("LRR-E-INTERNAL-SNAPSHOT", "constructor source plan differs"),
                "body" => ("LRR-E-INTERNAL-SNAPSHOT", "entered body key is stale"),
                _ => (
                    "LRR-E-UNSUPPORTED",
                    "constructor caller source revision differs from database",
                ),
            };
            assert_eq!(error.code, code, "{changed}: {error:?}");
            assert_eq!(error.message, message, "{changed}");
            assert_eq!(run.store.budget().handles(), handles, "{changed}");
            assert_eq!(run.frame.fuel.snapshot().used, fuel, "{changed}");
            assert_eq!(run.frame.pc(), &pc, "{changed}");
            assert_eq!(run.frame.counters.operations_started, 0, "{changed}");
            assert!(run.frame.cell(destination).is_err(), "{changed}");
            // Restore the exact captured publication between runs. A refused
            // preparation must not leave facts that block its later admission.
            db.finish_run().unwrap();
            db.publish_inputs(run.sources[0].prepared_inputs().clone())
                .unwrap();
            db.begin_run().unwrap();
            run.prepare_constructor(&mut db, &[])
                .into_ready()
                .unwrap()
                .unwrap();
            assert_eq!(run.store.budget().handles(), handles, "{changed}");
            assert_eq!(run.frame.fuel.snapshot().used, fuel, "{changed}");
            assert!(run.frame.cell(destination).is_err(), "{changed}");
            db.finish_run().unwrap();
        }
    }
    fn stock_constructor() -> (Database, RunMachine, PreparedSourceView, SlotId) {
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(LARGE.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("large-records.rs").unwrap();
        let unit = Digest::of_bytes(b"rustc-large-record-dispatch");
        let selection = SourceSelection {
            path: path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("stock source absent")
        };
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "f")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let mut run = RunMachine::start_with_sources(&mut db, body, 52, vec![view.share()])
            .into_ready()
            .unwrap()
            .unwrap();
        let node = run
            .frame
            .body
            .nodes()
            .iter()
            .find(|node| node.constructor_guard.is_some())
            .unwrap()
            .clone();
        // Isolate the unchanged source constructor prefix. Nominal let type
        // annotations and full body cleanup are explicitly outside this control.
        let mut scopes = vec![];
        let mut scope = Some(node.scope);
        while let Some(id) = scope {
            scopes.push(id);
            scope = run.frame.body.scopes()[id.0 as usize].parent;
        }
        for scope in scopes.into_iter().rev() {
            run.frame.enter(scope).unwrap();
        }
        run.frame.pc = run.frame.body.pc(node.id, Phase::Entry).unwrap();
        (db, run, view, node.value.unwrap())
    }
    fn require_large_prefix(db: &Database, run: &mut RunMachine, destination: SlotId) {
        assert_eq!(run.frame.counters.operations_started, 26);
        assert_eq!(run.frame.counters.operations_completed, 26);
        assert_eq!(run.frame.counters.exits_completed, 26);
        assert_eq!(run.frame.fuel.snapshot().used, 52);
        assert_eq!(run.frame.counters.started_by_opcode[0], 12);
        assert_eq!(run.frame.counters.started_by_opcode[27], 1);
        assert_eq!(run.frame.counters.started_by_opcode[28], 12);
        assert_eq!(run.frame.counters.started_by_opcode[29], 1);
        let ticket = run
            .store
            .move_from(run.frame.cell(destination).unwrap())
            .unwrap();
        let value = run.store.peek_ticket(ticket).unwrap();
        let descriptor = run.store.type_descriptor(TypeId(value.type_id)).unwrap();
        assert!(matches!(descriptor.key.as_ref(), TypeKey::Nominal { .. }));
        for ordinal in 0..12 {
            let cell = run.store.aggregate_field(ticket, ordinal).unwrap();
            let actual = run.store.read_scalar(cell).unwrap();
            assert_eq!(actual.type_id, 8, "original isize field context");
            assert_eq!(actual.payload, 0, "original literal spelling is zero");
        }
        // Cleanup is a harness-owned drain, not a claim of guest Drop dispatch.
        for _ in 0..12 {
            let field = run.store.aggregate_cleanup_next(ticket).unwrap().unwrap();
            run.store.discard(field).unwrap();
        }
        assert!(run.store.aggregate_cleanup_next(ticket).unwrap().is_none());
        run.store.discard(ticket).unwrap();
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Constant(_)
                | QueryKey::CurrentExpansionInput(_)
                | QueryKey::ExpansionInput(_)
        )));
    }
    #[test]
    fn rustc_large_record_nominal_context_refusal_preserves_requirements_and_ownership() {
        use crate::{
            ir::op::PrimitiveType,
            traits::term::{ParamKey, TyTerm},
        };
        // Original LARGE constructor and source-checked field schema. Injected
        // source requirements exercise LRR-specific admission, not a claim that
        // the unchanged nongeneric source declares these symbolic parameters.
        for (phase, terminal) in [
            ("prepare", false),
            ("start", false),
            ("finish", false),
            ("start", true),
            ("finish", true),
        ] {
            for primitive in [false, true] {
                let (mut db, mut run, _, destination) = stock_constructor();
                let guard = run.frame.body.nodes()[run.frame.pc().node as usize]
                    .constructor_guard
                    .unwrap();
                let source = if primitive {
                    TyTerm::Primitive(PrimitiveType::Bool)
                } else {
                    TyTerm::Param(ParamKey {
                        definition: run.frame.body.body().definition,
                        ordinal: 0,
                    })
                };
                if phase != "prepare" {
                    run.prepare_constructor(&mut db, &[])
                        .into_ready()
                        .unwrap()
                        .unwrap();
                    if phase == "finish" {
                        // Enter only the original guard/literal/field recipes;
                        // no source result has committed at this point.
                        for _ in 0..25 {
                            let outcome = db
                                .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
                                .into_ready()
                                .unwrap()
                                .unwrap();
                            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                                panic!("original nominal prefix thunk absent")
                            };
                            assert!(matches!(
                                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                                StepOutcome::Next(_)
                            ));
                        }
                    }
                }
                let original_source = run.frame.source_view(destination).unwrap();
                run.frame
                    .set_source_view(destination, Some(source.clone()))
                    .unwrap();
                let handles = run.store.budget().handles();
                let bytes = run.store.budget().bytes();
                let metadata = run.store.type_metadata_bytes();
                let fuel = run.frame.fuel.snapshot().used;
                let pc = run.frame.pc().clone();
                let started = run.frame.counters.operations_started;
                let completed = run.frame.counters.operations_completed;
                let error = if phase == "prepare" {
                    run.prepare_constructor(&mut db, &[])
                        .into_ready()
                        .unwrap()
                        .unwrap_err()
                } else if !terminal {
                    if phase == "start" {
                        run.frame.preflight_prepared_constructor(guard).unwrap_err()
                    } else {
                        run.frame
                            .preflight_constructor_finish(guard, destination, &run.store)
                            .unwrap_err()
                    }
                } else {
                    let outcome = db
                        .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
                        .into_ready()
                        .unwrap()
                        .unwrap();
                    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                        panic!("original nominal boundary thunk absent")
                    };
                    step::step(&mut run.frame, &mut run.store, recipe).unwrap_err()
                };
                assert_eq!(error.code, "LRR-E-GENERIC-BINDING", "{phase}");
                assert_eq!(
                    error.message, "nominal constructor result requires its source type binding",
                    "{phase}"
                );
                assert_eq!(
                    run.frame.source_view(destination).unwrap(),
                    Some(source.clone()),
                    "{phase}"
                );
                assert_eq!(run.store.budget().handles(), handles, "{phase}");
                assert_eq!(run.store.budget().bytes(), bytes, "{phase}");
                assert_eq!(run.store.type_metadata_bytes(), metadata, "{phase}");
                assert_eq!(run.frame.fuel.snapshot().used, fuel, "{phase}");
                assert_eq!(run.frame.pc(), &pc, "{phase}");
                assert_eq!(run.frame.counters.operations_started, started, "{phase}");
                assert_eq!(
                    run.frame.counters.operations_completed, completed,
                    "{phase}"
                );
                assert!(run.frame.cell(destination).is_err(), "{phase}");
                if terminal {
                    assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Faulted);
                    let outcome = db
                        .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
                        .into_ready()
                        .unwrap()
                        .unwrap();
                    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                        panic!("faulted original nominal boundary thunk absent")
                    };
                    let retry = step::step(&mut run.frame, &mut run.store, recipe).unwrap_err();
                    assert_eq!(retry.code, "LRR-E-INTERNAL-IR");
                    assert_eq!(retry.message, "terminal frame cannot execute again");
                    assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Faulted);
                    assert_eq!(run.frame.source_view(destination).unwrap(), Some(source));
                    assert_eq!(run.store.budget().handles(), handles);
                    assert_eq!(run.store.budget().bytes(), bytes);
                    assert_eq!(run.store.type_metadata_bytes(), metadata);
                    assert_eq!(run.frame.fuel.snapshot().used, fuel);
                    assert_eq!(run.frame.pc(), &pc);
                    assert_eq!(run.frame.counters.operations_started, started);
                    assert_eq!(run.frame.counters.operations_completed, completed);
                    assert!(run.frame.cell(destination).is_err());
                    if phase == "finish" {
                        // Only the harness drains the already transferred fields;
                        // no guest Drop or execution after Faulted is permitted.
                        for _ in 0..12 {
                            let field = run
                                .frame
                                .constructor_cleanup_next(guard, &mut run.store)
                                .unwrap()
                                .unwrap();
                            let value = run.store.peek_ticket(field).unwrap();
                            assert_eq!(value.type_id, 8);
                            assert_eq!(value.payload, 0);
                            run.store.discard(field).unwrap();
                        }
                        assert!(
                            run.frame
                                .constructor_cleanup_next(guard, &mut run.store)
                                .unwrap()
                                .is_none()
                        );
                    } else {
                        assert!(
                            run.frame
                                .constructor_cleanup_next(guard, &mut run.store)
                                .is_err()
                        );
                    }
                    db.finish_run().unwrap();
                    continue;
                }
                assert_eq!(run.frame.status(), crate::vm::frame::FrameStatus::Running);
                // Restore only the injected context. The failed pure preflight must
                // retain its ready facts/partial guard, so actual original
                // recipes can finish without replaying any transferred field.
                run.frame
                    .set_source_view(destination, original_source)
                    .unwrap();
                if phase == "prepare" {
                    run.prepare_constructor(&mut db, &[])
                        .into_ready()
                        .unwrap()
                        .unwrap();
                }
                while run.frame.counters.operations_completed < 26 {
                    let outcome = db
                        .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
                        .into_ready()
                        .unwrap()
                        .unwrap();
                    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                        panic!("restored original nominal thunk absent")
                    };
                    assert!(matches!(
                        step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                        StepOutcome::Next(_)
                    ));
                }
                require_large_prefix(&db, &mut run, destination);
                db.finish_run().unwrap();
            }
        }
    }
    #[test]
    fn rustc_large_record_dispatch_binds_fields_and_stops_at_exact_fuel_boundary() {
        let (mut db, mut run, _, destination) = stock_constructor();
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("selected prefix must stop at its charged fuel boundary")
        };
        assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
        assert_eq!(error.message, "guest opcode fuel exhausted");
        require_large_prefix(&db, &mut run, destination);
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_large_record_dispatch_reuses_matching_published_macro_context() {
        let (mut db, mut run, view, destination) = stock_constructor();
        let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
        let context = macros.context(&view).unwrap();
        let RunPoll::Fault(error) = run.poll_with_runtime_macros(&mut db, &[context]) else {
            panic!("selected prefix must stop at its charged fuel boundary")
        };
        assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
        assert_eq!(error.message, "guest opcode fuel exhausted");
        require_large_prefix(&db, &mut run, destination);
        db.finish_run().unwrap();
    }
    #[test]
    fn rustc_large_record_dispatch_refuses_missing_source_owner_before_guard_or_fuel() {
        let (mut db, mut run, _, destination) = stock_constructor();
        run.sources.clear();
        let handles = run.store.budget().handles();
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("missing owner must fault before constructor execution")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert_eq!(error.message, "constructor source owner absent");
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.frame.counters.operations_started, 0);
        assert_eq!(run.frame.fuel.snapshot().used, 0);
        assert!(run.frame.cell(destination).is_err());
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::TypeSchema(_) | QueryKey::FunctionSignature(_)
        )));
        db.finish_run().unwrap();
    }
}
