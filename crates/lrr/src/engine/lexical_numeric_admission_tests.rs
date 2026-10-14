//! Source-only lexical admission invalidation controls; execution pending.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    runtime::shape::TypeId,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{body_index::ExpressionKind, parse::ParseKey},
};
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
        Digest::of_bytes(b"LRR lexical numeric compiler"),
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
fn assert_no_child_execution_or_body_queries(
    db: &Database,
    main: &crate::source::manifest::BodyKey,
    machine: &RunMachine,
) {
    assert_eq!(machine.opcode_counters().unwrap().operations_started, 0);
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => body == main,
        QueryKey::Thunk(_) => false,
        _ => true,
    }));
}
fn assert_no_local_rows(
    db: &Database,
    plan: &crate::syntax::lexical_numeric_callees::LexicalFunctionPlan,
) {
    use crate::db::dependency::InputKey;
    let id = plan.signature().definition;
    for key in [
        InputKey::DefinitionSource(id),
        InputKey::DefinitionHeader(id),
        InputKey::DeclarationContext(id),
        InputKey::DefinitionBody(id),
    ] {
        assert!(
            db.published_input(&key).is_none(),
            "partially admitted local metadata"
        );
    }
}
#[test]
fn exact_owning_original_admission_retains_source_revision_and_private_certificate_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, discovery) = setup(ORIGINAL, edition);
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
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(machine)) =
            RunMachine::start_with_discovery(&mut db, main.clone(), 100000, discovery)
        else {
            panic!()
        };
        let plan = captured_original_plan(&mut db, &view, &machine);
        assert_no_local_rows(&db, &plan);
        let source_revision = db.source_revision();
        let QueryPoll::Ready(Ok(callee)) =
            crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
        else {
            panic!("genuine owning source refused")
        };
        assert_eq!(callee.body(), plan.item().body.as_ref());
        assert_eq!(callee.signature(), plan.signature());
        assert!(callee.retained_lexical_metadata_bytes() >= plan.metadata_bytes());
        assert_eq!(db.source_revision(), source_revision);
        callee.revalidate(&mut db).unwrap();
        assert_no_child_execution_or_body_queries(&db, &main, &machine);
        db.finish_run().unwrap();
    }
}
#[test]
fn copied_plan_wrong_captured_source_refuses_without_partial_local_publication() {
    let (mut db, view, discovery) = setup(ORIGINAL, 2024);
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
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(machine)) =
        RunMachine::start_with_discovery(&mut db, main.clone(), 100000, discovery)
    else {
        panic!()
    };
    let plan = captured_original_plan(&mut db, &view, &machine);
    let (_other_db, other_view, _other_discovery) =
        setup("fn main(){fn id_i8(n:i8)->i8{n}id_i8(1i8);}", 2024);
    let revision = db.revision();
    let QueryPoll::Ready(Err(error)) =
        crate::frontend::callables::admit_lexical_function(&mut db, &other_view, &plan.clone())
    else {
        panic!("copied plan switched owner")
    };
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert_eq!(db.revision(), revision);
    assert_no_local_rows(&db, &plan);
    assert_no_child_execution_or_body_queries(&db, &main, &machine);
    db.finish_run().unwrap();
}
#[test]
fn changed_derived_epoch_invalidates_plan_without_source_revision_advance() {
    let (mut db, view, discovery) = setup(ORIGINAL, 2024);
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
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(machine)) =
        RunMachine::start_with_discovery(&mut db, main.clone(), 100000, discovery)
    else {
        panic!()
    };
    let plan = captured_original_plan(&mut db, &view, &machine);
    let source_revision = db.source_revision();
    let revision = db.revision();
    db.finish_run().unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), source_revision);
    assert!(db.revision() > revision);
    db.begin_run().unwrap();
    let QueryPoll::Ready(Err(error)) =
        crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
    else {
        panic!("stale namespace epoch admitted")
    };
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert_no_local_rows(&db, &plan);
    assert_no_child_execution_or_body_queries(&db, &main, &machine);
    db.finish_run().unwrap();
}
#[test]
fn admitted_callable_revalidates_derived_epoch_even_when_definition_rows_match() {
    let (mut db, view, discovery) = setup(ORIGINAL, 2024);
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
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(machine)) =
        RunMachine::start_with_discovery(&mut db, main.clone(), 100000, discovery)
    else {
        panic!()
    };
    let plan = captured_original_plan(&mut db, &view, &machine);
    let QueryPoll::Ready(Ok(callee)) =
        crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
    else {
        panic!()
    };
    callee.revalidate(&mut db).unwrap();
    let source_revision = db.source_revision();
    let id = callee.identity();
    let rows = [
        crate::db::dependency::InputKey::DefinitionSource(id),
        crate::db::dependency::InputKey::DefinitionHeader(id),
        crate::db::dependency::InputKey::DeclarationContext(id),
        crate::db::dependency::InputKey::DefinitionBody(id),
    ]
    .map(|key| (key.clone(), db.input(&key).unwrap().fingerprint()));
    db.finish_run().unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), source_revision);
    for (key, fingerprint) in rows {
        assert_eq!(db.input(&key).unwrap().fingerprint(), fingerprint);
    }
    db.begin_run().unwrap();
    let error = callee.revalidate(&mut db).unwrap_err();
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert_no_child_execution_or_body_queries(&db, &main, &machine);
    db.finish_run().unwrap();
}
#[test]
fn local_admission_metadata_quota_refuses_atomically_before_child_signature_or_body() {
    let (mut db, view, discovery) = setup(ORIGINAL, 2024);
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
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(machine)) =
        RunMachine::start_with_discovery(&mut db, main.clone(), 100000, discovery)
    else {
        panic!()
    };
    let plan = captured_original_plan(&mut db, &view, &machine);
    db.finish_run().unwrap();
    let budget = db.cache_usage().unwrap().memo_bytes;
    db.begin_run_with_budgets(crate::engine::telemetry::CacheBudgets {
        query_bytes: budget,
        ..Default::default()
    })
    .unwrap();
    let revision = db.revision();
    let QueryPoll::Ready(Err(error)) =
        crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
    else {
        panic!("local admission exceeded metadata cap")
    };
    assert_eq!(error.code, "LRR-E-METADATA-LIMIT");
    assert_eq!(db.revision(), revision);
    assert_no_local_rows(&db, &plan);
    assert_no_child_execution_or_body_queries(&db, &main, &machine);
    assert!(db.trace.events.iter().all(|event| !matches!(event.key, QueryKey::FunctionSignature(id) if id == plan.signature().definition)));
    db.finish_run().unwrap();
}

#[test]
fn unchanged_block_fn_retains_exact_local_header_but_does_not_replace_function_value_call_identity()
{
    use crate::core::digest::StableEncode;
    let original =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/expr/block-fn.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let parsed = crate::syntax::parse::ParsedFile::parse(
            &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
            profile,
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"LRR original block fn exact owner"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        let owner = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "test_fn")
            .unwrap();
        let handle = owner.body.as_ref().unwrap();
        let index = parsed.index_body(handle, Default::default()).unwrap();
        let facts = parsed
            .entered_macro_facts(handle, index.cfg(), &owner.scope)
            .unwrap();
        let local = facts
            .items
            .iter()
            .find(|row| row.item.def_key.declared_name == "ten")
            .unwrap();
        let body = local.item.body.as_ref().unwrap();
        assert_eq!(local.item.header.profile.edition, edition);
        assert_eq!(
            Digest::of_bytes(
                &original.as_bytes()[body.range.start as usize..body.range.end as usize]
            ),
            body.body_hash
        );
        let at = u32::try_from(original.find("rs()").unwrap()).unwrap() - handle.range.start;
        assert!(
            !index
                .nodes()
                .iter()
                .any(|node| node.range.start == at && node.range.end == at + 4)
        );
        let node = index
            .nodes()
            .iter()
            .find(|node| {
                node.role == crate::syntax::body_index::NodeRole::Expression(ExpressionKind::Macro)
            })
            .unwrap();
        let error = parsed
            .lexical_numeric_function_plan(
                handle,
                &index,
                &index.pc(node.id, Phase::Entry).unwrap(),
                &owner.header,
                &crate::syntax::outline::DeclarationContext {
                    scope: owner.scope.clone(),
                    range: owner.declaration_span,
                },
                local.item.def_key.fingerprint(),
                "ten",
                1,
                1,
            )
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
        assert_eq!(error.message, "lexical call exact syntax absent");
        // The unchanged runtime contract calls a function value rs within an
        // opaque assert_eq! input. Capturing ten's exact declaration cannot
        // turn the original opaque macro node into a direct lexical call.
        // No guest opcode/DB body/thunk execution or whole-fixture pass here.
    }
}

#[test]
fn original_deferred_lexical_signature_has_separate_checked_numeric_contract() {
    use crate::frontend::call_contracts::{
        ArgumentHintPreparation, ArgumentType, ContractPreparation, prepare_argument_hint,
        prepare_contracts,
    };
    use crate::ir::op::PrimitiveType;
    use crate::traits::term::TyTerm;
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, discovery) = setup(ORIGINAL, edition);
        let main = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(machine)) =
            RunMachine::start_with_discovery(&mut db, main.clone(), 100_000, discovery)
        else {
            panic!("exact original startup")
        };
        let plan = captured_original_plan(&mut db, &view, &machine);
        assert!(plan.signature().parameters[0].symbolic.is_none());
        assert!(plan.signature().result_symbolic.is_none());
        let callee = crate::frontend::callables::admit_lexical_function(&mut db, &view, &plan)
            .into_ready()
            .expect("owning admission input")
            .unwrap();
        let original = callee.signature().clone();
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) =
            prepare_argument_hint(&mut db, &callee, &graph, &[&view], 0)
        else {
            panic!("certified primitive hint")
        };
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].source, TyTerm::Primitive(PrimitiveType::I8));
        let actual = ArgumentType {
            source: TyTerm::Primitive(PrimitiveType::I8),
            descriptor: TypeId(4),
        };
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(mut contracts))) =
            prepare_contracts(&mut db, &callee, &graph, &[&view], &[], None, &[actual])
        else {
            panic!("closed source contract")
        };
        assert_eq!(callee.signature(), &original);
        assert!(contracts.normalized_deferred_return.is_some());
        assert!(crate::vm::context::GenericEnv::capture(&original, &contracts).is_err());
        let env =
            crate::vm::context::GenericEnv::capture_current(&mut db, &callee, &contracts).unwrap();
        assert_eq!(env.definition(), &original);
        contracts.source_parameters[0] = TyTerm::Primitive(PrimitiveType::Bool);
        assert!(
            crate::vm::context::GenericEnv::capture_current(&mut db, &callee, &contracts).is_err()
        );
        assert_no_child_execution_or_body_queries(&db, &main, &machine);
        db.finish_run().unwrap();
    }
}
