//! LRR installed-use authority controls, outside unchanged upstream originals.
//! Uncompiled/unexecuted until the sole validator runs the assembled candidate.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{dependency::InputKey, key::OutlineKey},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn setup(text: &str, edition: u16) -> (Database, PreparedSourceView, SourceDiscovery) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("lexical-numeric-original.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let unit = Digest::of_bytes(b"LRR lexical numeric source");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"LRR lexical numeric discovery"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR lexical numeric declarative"),
        crate::expand::source_declarative::engine_version(),
        std::sync::Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                .unwrap(),
        ),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            _ => panic!("lexical source discovery requires unsupported authority"),
        }
    }
    db.begin_expansion_epoch(1).unwrap();
    (db, view, discovery)
}

fn captured_original_plan(
    db: &mut Database,
    view: &PreparedSourceView,
    machine: &RunMachine,
) -> crate::syntax::lexical_numeric_callees::LexicalFunctionPlan {
    use crate::core::digest::StableEncode;
    let owner = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap();
    let body = owner.body.as_ref().unwrap();
    let QueryPoll::Ready(Ok(outcome)) = db.fetch(
        &SyntaxProvider,
        QueryKey::ParseFile(view.selection().outline.parse.clone()),
    ) else {
        panic!()
    };
    let QueryValue::ParseFile(parsed) = outcome.value.as_ref() else {
        panic!()
    };
    let facts = parsed
        .entered_macro_facts(body, machine.frame.body.cfg(), &owner.scope)
        .unwrap();
    let selected = facts
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == "id_i8")
        .unwrap();
    let call = "id_i8(j)";
    let at = u32::try_from(ORIGINAL.find(call).unwrap()).unwrap() - body.range.start;
    let node = machine
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| node.range.start == at && node.range.end == at + 8)
        .unwrap();
    parsed
        .lexical_numeric_function_plan(
            body,
            &machine.frame.body,
            &machine.frame.body.pc(node.id, Phase::Entry).unwrap(),
            &owner.header,
            &crate::syntax::outline::DeclarationContext {
                scope: owner.scope.clone(),
                range: owner.declaration_span,
            },
            selected.item.def_key.fingerprint(),
            "id_i8",
            db.source_revision(),
            db.revision(),
        )
        .unwrap()
}

fn start(text: &str, edition: u16) -> (Database, PreparedSourceView, RunMachine) {
    let (mut db, view, discovery) = setup(text, edition);
    let main = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    // Startup publishes the selected body's environment; the run (which
    // forbids source preparation) begins only after it, as in the CLI.
    let machine = RunMachine::start_with_discovery(&mut db, main, 100_000, discovery)
        .into_ready()
        .expect("source readiness")
        .expect("genuine discovery startup");
    db.begin_run().unwrap();
    (db, view, machine)
}
fn thunk(db: &mut Database, pc: &PcKey) -> crate::ir::recipe::VerifiedRecipe {
    let outcome = db
        .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
        .into_ready()
        .expect("thunk input")
        .expect("source thunk");
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("thunk result kind")
    };
    recipe.as_ref().clone()
}
fn publish_new_type_seal(db: &mut Database, view: &PreparedSourceView) {
    // A real second, separately sealed namespace of the SAME captured source,
    // graph and epoch. No DiscoveryInput or seal constructor is forged here.
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest::of_bytes(b"LRR lexical numeric discovery"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR lexical numeric declarative"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &view.selection().outline.parse.profile,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    match discovery.poll().expect("actual Type discovery") {
        SourceDiscoveryStep::Sealed(_) => {}
        // No fixture macro or guest execution is authorized by this test.
        _ => panic!("simple source unexpectedly needs expansion/module authority"),
    }
    let key = InputKey::DiscoverySeal(discovery.key().clone());
    assert!(
        db.published_input(&key).is_none(),
        "new namespace must add membership"
    );
    discovery
        .publish(db)
        .expect("genuine active-run derived publication");
    assert!(db.published_input(&key).is_some());
}
#[test]
fn genuine_same_epoch_namespace_addition_refuses_cached_normal_dispatch_before_guest_work() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, mut run) = start("fn main(){let x=1i32;}", edition);
        let pc = run.frame.pc.clone();
        let raw = thunk(&mut db, &pc);
        let recipe = raw.clone().with_namespace_witness(&db).unwrap();
        let id = run
            .installed
            .install_prepared(&db, pc.clone(), &recipe)
            .unwrap();
        let mut ordinary = InstalledRecipes::new(
            db.source_revision(),
            super::super::telemetry::INSTALLED_LRIR_BUDGET,
        )
        .unwrap();
        let ordinary_id = ordinary.install_prepared(&db, pc.clone(), &raw).unwrap();
        let old = db.lexical_namespace_stamps().unwrap();
        let source_revision = db.source_revision();
        let revision = db.revision();
        let epoch = db.published_input(&InputKey::ExpansionEpoch).unwrap().1;
        let fuel = run.fuel_snapshot();
        let actions = run.action_snapshot();
        let owners = run.store.owner_inventory_for_test();
        let handles = run.store.budget().handles();
        let scopes = run.frame.active_scopes().to_vec();
        let resume = run.frame.resume_operation_index();
        assert_eq!(run.opcode_counters().unwrap().operations_started, 0);
        publish_new_type_seal(&mut db, &view);
        assert_eq!(db.source_revision(), source_revision);
        assert!(db.revision() > revision);
        assert_eq!(
            db.published_input(&InputKey::ExpansionEpoch).unwrap().1,
            epoch
        );
        let current = db.lexical_namespace_stamps().unwrap();
        assert!(current.len() > old.len());
        assert!(
            old.iter()
                .all(|(key, stamp)| current.get(key) == Some(stamp)),
            "old fingerprints must be unchanged"
        );
        assert_eq!(
            run.installed.get_current(&db, id).unwrap_err().code,
            "LRR-E-NAMESPACE-STALE"
        );
        let error = run
            .installed
            .resolve(&mut db, pc.clone())
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-NAMESPACE-STALE");
        assert_eq!(
            recipe.clone().with_namespace_witness(&db).unwrap_err().code,
            "LRR-E-NAMESPACE-STALE"
        );
        let mut fresh = InstalledRecipes::new(
            db.source_revision(),
            super::super::telemetry::INSTALLED_LRIR_BUDGET,
        )
        .unwrap();
        assert_eq!(
            fresh
                .install_prepared(&db, pc.clone(), &recipe)
                .unwrap_err()
                .code,
            "LRR-E-NAMESPACE-STALE"
        );
        assert_eq!(fresh.len(), 0);
        assert_eq!(fresh.metadata_bytes(), 0);
        assert_eq!(
            ordinary
                .resolve(&mut db, pc.clone())
                .into_ready()
                .unwrap()
                .unwrap(),
            ordinary_id
        );
        assert!(
            ordinary.get_current(&db, ordinary_id).is_ok(),
            "ordinary caching remains enabled"
        );
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("cached sensitive recipe executed after namespace publication")
        };
        assert_eq!(error.code, "LRR-E-NAMESPACE-STALE");
        assert_eq!(run.frame.pc, pc);
        assert_eq!(run.frame.resume_operation_index(), resume);
        assert_eq!(run.frame.active_scopes(), scopes);
        assert_uncharged(run.fuel_snapshot(), fuel);
        assert_eq!(run.action_snapshot(), actions);
        assert_eq!(run.store.owner_inventory_for_test(), owners);
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.opcode_counters().unwrap().operations_started, 0);
        assert_eq!(run.opcode_counters().unwrap().exits_completed, 0);
        db.finish_run().unwrap();
    }
}
#[test]
fn physical_sidecar_retains_accounted_shared_ownership_without_changing_stable_recipe() {
    let (mut db, _view, run) = start("fn main(){}", 2024);
    let raw = thunk(&mut db, &run.frame.pc);
    let bytes = raw.metadata_bytes();
    let fingerprint = raw.recipe().fingerprint();
    let admitted = raw.with_namespace_witness(&db).unwrap();
    assert!(admitted.metadata_bytes() > bytes);
    assert_eq!(admitted.recipe().fingerprint(), fingerprint);
    let retained = admitted.metadata_bytes();
    let cloned = admitted.clone().with_namespace_witness(&db).unwrap();
    assert_eq!(
        cloned.metadata_bytes(),
        retained,
        "same current sidecar is not charged twice"
    );
    assert_eq!(cloned.recipe().fingerprint(), fingerprint);
    db.finish_run().unwrap();
}
#[test]
fn genuine_original_lexical_definition_publication_does_not_false_stale_namespace() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, mut run) = start(ORIGINAL, edition);
        // The unchanged original is used only to derive its actual nested
        // id_i8 source admission; its body/main runtime contract is unexecuted.
        let plan = captured_original_plan(&mut db, &view, &run);
        let pc = run.frame.pc.clone();
        let recipe = thunk(&mut db, &pc).with_namespace_witness(&db).unwrap();
        let id = run
            .installed
            .install_prepared(&db, pc.clone(), &recipe)
            .unwrap();
        let old = db.lexical_namespace_stamps().unwrap();
        let revision = db.revision();
        let source_revision = db.source_revision();
        let definition = plan.signature().definition;
        let keys = [
            InputKey::DefinitionSource(definition),
            InputKey::DefinitionHeader(definition),
            InputKey::DefinitionBody(definition),
            InputKey::DeclarationContext(definition),
        ];
        assert!(keys.iter().all(|key| db.published_input(key).is_none()));
        let callee = crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
            .into_ready()
            .expect("owning admission inputs")
            .expect("actual lexical admission");
        assert_eq!(callee.signature(), plan.signature());
        assert!(keys.iter().all(|key| db.published_input(key).is_some()));
        assert!(db.revision() > revision);
        assert_eq!(db.source_revision(), source_revision);
        assert_eq!(db.lexical_namespace_stamps().unwrap(), old);
        assert!(run.installed.get_current(&db, id).is_ok());
        assert_eq!(
            run.installed
                .resolve(&mut db, pc)
                .into_ready()
                .unwrap()
                .unwrap(),
            id
        );
        assert_eq!(run.opcode_counters().unwrap().operations_started, 0);
        assert_eq!(run.opcode_counters().unwrap().exits_completed, 0);
        db.finish_run().unwrap();
    }
}

fn assert_uncharged(after: crate::vm::steps::StepSnapshot, before: crate::vm::steps::StepSnapshot) {
    assert_eq!(after.used, before.used);
}

const CONST_ARRAY_ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
);
// An explicitly LRR-specific source supplement, appended outside the original.
// The original ARR/const-fn/main declarations and attributes remain unchanged.
// Primitive fields avoid inventing user Drop, Copy or trait-selection authority.
const RESUME_ADAPTER: &str = r#"
const RESUME_LARGE: [u8; 1024] = [0; 1024];
fn namespace_resume_probe() {
    fn resume_id(n: i8) -> i8 { n }
    { let a = RESUME_LARGE; }
    if false { resume_id(1i8); }
}
"#;

fn start_resume(edition: u16) -> (Database, PreparedSourceView, RunMachine, String) {
    let text = format!("{CONST_ARRAY_ORIGINAL}\n{RESUME_ADAPTER}");
    // This existing helper publishes authentic prepared owning Value and Type
    // discovery, including the unchanged original's attribute preparation.
    let (mut db, view, _) =
        crate::frontend::constant_construction::tests::world(&text, "RESUME_LARGE", edition);
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "namespace_resume_probe")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let run = RunMachine::start_with_sources(&mut db, body, 100_000, vec![view.share()])
        .into_ready()
        .expect("resume source input")
        .expect("actual source startup");
    (db, view, run, text)
}

/// One real installed/source-prepared VM chunk on the actual RunMachine. Normal
/// poll loops over Dispatch internally, so a test driver must stop at this
/// existing chunk boundary to publish between dispatches. This never writes a
/// PC, frame slot, activation, descriptor, source proof or pending index.
fn actual_chunk(db: &mut Database, run: &mut RunMachine) -> StepOutcome {
    let pc = run.frame.pc().clone();
    let id = if run.installed.contains_pc(&pc) {
        run.installed.resolve(db, pc).into_ready().unwrap().unwrap()
    } else {
        let prepared = run
            .prepare_source_constant_recipe(db)
            .into_ready()
            .expect("constant preparation input")
            .expect("source constant preparation");
        let raw = prepared.unwrap_or_else(|| thunk(db, &pc));
        // This records currentness on the actual cleanup recipe, not semantic
        // name permission. Its ordinary source verifier already admitted it.
        let recipe = if raw
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::ScopeLeave { .. }))
        {
            raw.with_namespace_witness(db).unwrap()
        } else {
            raw
        };
        run.installed.install_prepared(db, pc, &recipe).unwrap()
    };
    let recipe = run.installed.get_current(db, id).unwrap();
    if recipe
        .recipe()
        .operations
        .iter()
        .any(|op| matches!(op, Op::SourceConstant { .. }))
    {
        let plan = run
            .constant_plans
            .get(run.frame.pc())
            .expect("actual retained source plan");
        let construction = plan.construction(db, &run.sources).unwrap();
        let views = run.sources.iter().collect::<Vec<_>>();
        let proof = plan
            .proof(
                &construction,
                &run.frame,
                &mut run.store,
                &run.sources,
                &views,
            )
            .unwrap();
        step::step_with_constant(&mut run.frame, &mut run.store, recipe, Some(&proof)).unwrap()
    } else {
        step::step(&mut run.frame, &mut run.store, recipe).unwrap()
    }
}

fn reach_nonzero_cleanup(db: &mut Database, run: &mut RunMachine) -> (u8, u64) {
    for _ in 0..512 {
        let pc = run.frame.pc().clone();
        let completed = run.frame.counters.operations_completed;
        let outcome = actual_chunk(db, run);
        match outcome {
            StepOutcome::Dispatch
                if run.frame.has_cleanup_activation() && run.frame.resume_operation_index() > 0 =>
            {
                assert_eq!(run.frame.pc(), &pc);
                assert_eq!(run.frame.resume_operation_index(), 1);
                assert_eq!(run.frame.counters.operations_completed, completed + 1);
                let id = run.installed.resolve(db, pc).into_ready().unwrap().unwrap();
                let recipe = run.installed.get_current(db, id).unwrap();
                let [prefix @ Op::Unit { .. }, Op::ScopeLeave { .. }] =
                    recipe.recipe().operations.as_ref()
                else {
                    panic!("actual nested block cleanup did not retain Unit/ScopeLeave")
                };
                let tag = prefix.opcode_tag();
                return (tag, run.frame.counters.started_by_opcode[usize::from(tag)]);
            }
            StepOutcome::Dispatch | StepOutcome::Next(_) => {}
            _ => {
                panic!("resume adapter reached unexpected call/panic/terminal before cleanup chunk")
            }
        }
    }
    panic!("real source did not reach bounded nonzero cleanup suspension")
}

fn publish_resume_namespace(db: &mut Database, view: &PreparedSourceView) {
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Macro,
        Digest::of_bytes(b"result type current world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"result type declarative"),
        Digest::of_bytes(b"result type compiler"),
        Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &view.selection().outline.parse.profile,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    match discovery
        .poll()
        .expect("actual Macro namespace preparation")
    {
        SourceDiscoveryStep::Sealed(_) => {}
        _ => panic!("unchanged const declarations need additional preparation authority"),
    }
    let key = InputKey::DiscoverySeal(discovery.key().clone());
    assert!(db.published_input(&key).is_none());
    discovery
        .publish(db)
        .expect("genuine same-epoch derived namespace publication");
    assert!(db.published_input(&key).is_some());
}

fn resume_lexical_plan(
    db: &mut Database,
    view: &PreparedSourceView,
    run: &RunMachine,
    text: &str,
) -> crate::syntax::lexical_numeric_callees::LexicalFunctionPlan {
    let owner = view
        .outline()
        .items
        .iter()
        .find(|item| {
            item.body
                .as_ref()
                .is_some_and(|body| body.key() == *run.frame.body.body())
        })
        .expect("actual entered source owner");
    let handle = owner.body.as_ref().unwrap();
    let parsed = db
        .fetch(
            &SyntaxProvider,
            QueryKey::ParseFile(view.selection().outline.parse.clone()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        panic!("source parse variant")
    };
    let facts = parsed
        .entered_macro_facts(handle, run.frame.body.cfg(), &owner.scope)
        .unwrap();
    let local = facts
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == "resume_id")
        .unwrap();
    let call = "resume_id(1i8)";
    let at = u32::try_from(text.find(call).unwrap()).unwrap() - handle.range.start;
    let end = at + u32::try_from(call.len()).unwrap();
    let node = run
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| node.range.start == at && node.range.end == end)
        .unwrap();
    parsed
        .lexical_numeric_function_plan(
            handle,
            &run.frame.body,
            &run.frame.body.pc(node.id, Phase::Entry).unwrap(),
            &owner.header,
            &crate::syntax::outline::DeclarationContext {
                scope: owner.scope.clone(),
                range: owner.declaration_span,
            },
            local.item.def_key.fingerprint(),
            "resume_id",
            db.source_revision(),
            db.revision(),
        )
        .unwrap()
}

#[test]
fn actual_nonzero_cleanup_resume_refuses_namespace_addition_without_repeating_prior_effects() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, mut run, _) = start_resume(edition);
        let (prefix_tag, prefix_count) = reach_nonzero_cleanup(&mut db, &mut run);
        let pc = run.frame.pc().clone();
        let old = db.lexical_namespace_stamps().unwrap();
        let revision = db.revision();
        let source_revision = db.source_revision();
        let epoch = db.published_input(&InputKey::ExpansionEpoch).unwrap().1;
        let fuel = run.fuel_snapshot();
        let actions = run.action_snapshot();
        let counters = serde_json::to_value(run.opcode_counters().unwrap()).unwrap();
        let owners = run.store.owner_inventory_for_test();
        let handles = run.store.budget().handles();
        let bytes = run.store.budget().bytes();
        let scopes = run.frame.active_scopes().to_vec();
        publish_resume_namespace(&mut db, &view);
        assert_eq!(db.source_revision(), source_revision);
        assert!(db.revision() > revision);
        assert_eq!(
            db.published_input(&InputKey::ExpansionEpoch).unwrap().1,
            epoch
        );
        let now = db.lexical_namespace_stamps().unwrap();
        assert!(now.len() > old.len());
        assert!(old.iter().all(|(key, stamp)| now.get(key) == Some(stamp)));
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("stale partial recipe resumed guest work")
        };
        assert_eq!(error.code, "LRR-E-NAMESPACE-STALE");
        assert_uncharged(run.fuel_snapshot(), fuel);
        assert_eq!(
            serde_json::to_value(run.opcode_counters().unwrap()).unwrap(),
            counters
        );
        assert_eq!(run.frame.pc(), &pc);
        assert_eq!(run.frame.resume_operation_index(), 1);
        assert!(run.frame.has_cleanup_activation());
        assert_eq!(
            run.frame.counters.started_by_opcode[usize::from(prefix_tag)],
            prefix_count
        );
        assert_eq!(run.frame.active_scopes(), scopes);
        assert_eq!(run.action_snapshot(), actions);
        assert_eq!(run.store.owner_inventory_for_test(), owners);
        assert_eq!(run.store.budget().handles(), handles);
        assert_eq!(run.store.budget().bytes(), bytes);
        db.finish_run().unwrap();
    }
}

fn finish_resume(db: &mut Database, run: &mut RunMachine) -> RunReport {
    match run.poll(db) {
        RunPoll::Completed(report) => {
            assert!(matches!(report.value, ValueSummary::Unit));
            report
        }
        RunPoll::Fault(error) => panic!(
            "real partial recipe refused {}: {}",
            error.code, error.message
        ),
        RunPoll::NeedInput(input) => panic!("resume lacks actual preparation {input:?}"),
        RunPoll::Panicked(panic) => panic!("unexpected guest panic {}", panic.panic.description()),
    }
}

#[test]
fn actual_nonzero_cleanup_resume_allows_genuine_lexical_rows_and_matches_unchanged_control() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut baseline_db, _, mut baseline, _) = start_resume(edition);
        let (baseline_tag, baseline_prefix) =
            reach_nonzero_cleanup(&mut baseline_db, &mut baseline);
        assert!(matches!(
            actual_chunk(&mut baseline_db, &mut baseline),
            StepOutcome::Dispatch
        ));
        assert_eq!(baseline.frame.resume_operation_index(), 1);
        assert_eq!(
            baseline.frame.counters.started_by_opcode[usize::from(baseline_tag)],
            baseline_prefix
        );
        let control = finish_resume(&mut baseline_db, &mut baseline);

        let (mut db, view, mut run, text) = start_resume(edition);
        let (prefix_tag, prefix_count) = reach_nonzero_cleanup(&mut db, &mut run);
        let plan = resume_lexical_plan(&mut db, &view, &run, &text);
        let rows = [
            InputKey::DefinitionSource(plan.signature().definition),
            InputKey::DefinitionHeader(plan.signature().definition),
            InputKey::DefinitionBody(plan.signature().definition),
            InputKey::DeclarationContext(plan.signature().definition),
        ];
        assert!(rows.iter().all(|key| db.published_input(key).is_none()));
        let old = db.lexical_namespace_stamps().unwrap();
        let source_revision = db.source_revision();
        let revision = db.revision();
        let callee = crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
            .into_ready()
            .expect("real lexical metadata input")
            .expect("genuine lexical function admission");
        assert_eq!(callee.signature(), plan.signature());
        assert!(rows.iter().all(|key| db.published_input(key).is_some()));
        assert!(db.revision() > revision);
        assert_eq!(db.source_revision(), source_revision);
        assert_eq!(db.lexical_namespace_stamps().unwrap(), old);
        let started = run.frame.counters.operations_started;
        let completed = run.frame.counters.operations_completed;
        assert!(matches!(
            actual_chunk(&mut db, &mut run),
            StepOutcome::Dispatch
        ));
        assert_eq!(run.frame.resume_operation_index(), 1);
        assert_eq!(run.frame.counters.operations_started, started);
        assert_eq!(run.frame.counters.operations_completed, completed);
        assert_eq!(
            run.frame.counters.started_by_opcode[usize::from(prefix_tag)],
            prefix_count
        );
        let report = finish_resume(&mut db, &mut run);
        assert_eq!(
            serde_json::to_value(report.opcode_counters).unwrap(),
            serde_json::to_value(control.opcode_counters).unwrap()
        );
        assert_eq!(run.fuel_snapshot(), baseline.fuel_snapshot());
        assert_eq!(run.action_snapshot(), baseline.action_snapshot());
        assert_eq!(
            run.store.budget().handles(),
            baseline.store.budget().handles()
        );
        assert_eq!(run.store.budget().bytes(), baseline.store.budget().bytes());
        assert_eq!(
            run.store.owner_inventory_for_test(),
            baseline.store.owner_inventory_for_test()
        );
        assert!(!run.frame.has_cleanup_activation());
        assert!(db.trace.events.iter().all(|event| !matches!(event.key,
            QueryKey::BodyStructure(ref body) if body.definition == plan.signature().definition)));
        db.finish_run().unwrap();
        baseline_db.finish_run().unwrap();
    }
}
