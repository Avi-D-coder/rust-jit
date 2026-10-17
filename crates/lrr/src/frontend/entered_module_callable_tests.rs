//! Source-admission controls on complete official roots. No guest execution.
use super::*;
use crate::{
    db::trace::Action,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{callables::CallSelection, compile_input_catalog::CompileInputCatalog},
    ir::pc::{NodeId, PcKey},
    source::changes::PreparedSourceView,
    syntax::body_index::{ExpressionKind, NodeRole},
};
use ra_ap_syntax::{AstNode, ast};

const AUTO: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/auto-traits/auto-is-contextual.rs"
);
const NESTED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual module source admission: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual module source authority: {input:?}"),
    }
}
fn world(
    text: &str,
    edition: u16,
    retained_caller: bool,
    retained_target: Option<&str>,
) -> (Database, Arc<PreparedSourceLocalPlan>) {
    world_entry(text, edition, retained_caller, retained_target, "main")
}
fn world_entry(
    text: &str,
    edition: u16,
    retained_caller: bool,
    retained_target: Option<&str>,
    entry: &str,
) -> (Database, Arc<PreparedSourceLocalPlan>) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"official selected module admission control"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"module admission compiler"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => break,
            _ => panic!("actual module discovery requires unsupported authority"),
        }
    }
    discovery.publish(&mut db).unwrap();
    if let Some(target) = retained_target {
        let CallSelection::Source(target) = ready(discovery.entry_function(&mut db, target)) else {
            panic!("actual retained target is not a source function")
        };
        retain(&mut db, &discovery, &source, target.identity());
    }
    let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, entry)) else {
        panic!("actual complete root main is not a source function")
    };
    let body = if retained_caller {
        retain(&mut db, &discovery, &source, main.identity())
    } else {
        main.body().unwrap().key()
    };
    let parent = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
    (db, parent)
}

#[test]
fn official_generated_constructor_is_canonical_kind_fact_and_never_function_authority() {
    // The full official source is unchanged. This appended observation reads
    // its real generated unit-struct constructor, without executing it.
    let source = format!("{AUTO}\nfn lrr_constructor_observation(){{S;}}\n");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, parent) =
            world_entry(&source, edition, false, None, "lrr_constructor_observation");
        let paths = parent
            .context()
            .source()
            .index()
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        let key = QueryKey::EnteredValue(PcKey {
            body: parent.context().source().index().body().clone(),
            node: paths[0].id.0,
            phase: 0,
        });
        let start = db.trace.events.len();
        let value = ready(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::EnteredValue(goal) = value.value.as_ref() else {
            panic!()
        };
        assert!(
            goal.pending().is_none(),
            "actual constructor Value pending: {goal:?}"
        );
        let selected = goal
            .declaration()
            .expect("real generated constructor declaration");
        let header = db
            .input(&InputKey::DefinitionHeader(selected.identity()))
            .unwrap();
        let InputValue::DefinitionHeader(header) = header.as_ref() else {
            panic!()
        };
        let canonical = ready(canonical_module_declaration(
            &mut db,
            &parent,
            selected.definition(),
            selected.source(),
            header,
            selected.context(),
        ));
        let CanonicalModulePreparation::Ready(canonical) = canonical else {
            panic!("real generated constructor canonical facts pending")
        };
        assert_eq!(canonical.definition().item_kind, ItemKind::Struct);
        let guard = Arc::new(ready(db.prepare_input_guard(
            &SyntaxProvider,
            &key,
            value.fingerprint,
        )));
        match ready(prepare_entered_module_function(
            &mut db,
            Arc::clone(&parent),
            selected,
            guard,
        )) {
            ModuleFunctionPreparation::Pending(ValueGoalDemand::BindingClassification(id)) => {
                assert_eq!(id, selected.identity());
            }
            _ => panic!("non-function constructor granted source function authority"),
        }
        no_executable_work(&db, start);
    }
}
fn retain(
    db: &mut Database,
    discovery: &SourceDiscovery,
    source: &PreparedSourceView,
    identity: Digest,
) -> crate::source::manifest::BodyKey {
    let prepared = discovery
        .prepare_body_syntax(source.selection(), identity)
        .unwrap();
    let input = ready(crate::db::retained_syntax::RetainedSyntaxInput::prepare(
        db,
        prepared,
        discovery.key().clone(),
    ));
    ready(db.publish_retained_syntax(Arc::clone(&input)));
    ready(db.publish_retained_body(input, identity))
}
fn emit(original: &str, edition: u16, wrapper: bool) -> String {
    let edition = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        2024 => ra_ap_syntax::Edition::Edition2024,
        _ => panic!("invalid module control edition"),
    };
    let parsed = ra_ap_syntax::SourceFile::parse(original, edition);
    assert!(parsed.errors().is_empty());
    let mut attributes = String::new();
    let mut items = String::new();
    let mut previous = 0;
    for attribute in parsed
        .tree()
        .syntax()
        .children()
        .filter_map(ast::Attr::cast)
    {
        let span = crate::syntax::ast_adapter::range(attribute.syntax().text_range());
        items.push_str(&original[previous..span.start as usize]);
        attributes.push_str(&original[span.start as usize..span.end as usize]);
        attributes.push('\n');
        previous = span.end as usize;
    }
    items.push_str(&original[previous..]);
    format!(
        "{attributes}macro_rules! emit{{()=>{{{items}}}}}{}",
        if wrapper {
            "macro_rules! wrap{()=>{emit!();}}wrap!();"
        } else {
            "emit!();"
        }
    )
}
fn calls(parent: &PreparedSourceLocalPlan) -> Vec<NodeId> {
    parent
        .context()
        .source()
        .index()
        .nodes()
        .iter()
        .filter_map(|node| {
            (node.role == NodeRole::Expression(ExpressionKind::Call)).then_some(node.id)
        })
        .collect()
}
fn admit(
    db: &mut Database,
    parent: &Arc<PreparedSourceLocalPlan>,
    call: NodeId,
) -> Arc<EnteredModuleAdmission> {
    let index = parent.context().source().index();
    let node = index.nodes().get(call.0 as usize).unwrap();
    assert_eq!(node.role, NodeRole::Expression(ExpressionKind::Call));
    let path = node.operands[0];
    let key = QueryKey::EnteredValue(PcKey {
        body: index.body().clone(),
        node: path.0,
        phase: 0,
    });
    let value = ready(db.fetch(&SyntaxProvider, key.clone()));
    let QueryValue::EnteredValue(goal) = value.value.as_ref() else {
        panic!()
    };
    assert!(
        goal.pending().is_none(),
        "actual module base is pending: {goal:?}"
    );
    let declaration = goal.declaration().expect("actual Value module declaration");
    assert!(declaration.owner().is_none());
    let guard = Arc::new(ready(db.prepare_input_guard(
        &SyntaxProvider,
        &key,
        value.fingerprint,
    )));
    match ready(prepare_entered_module_function(
        db,
        Arc::clone(parent),
        declaration,
        guard,
    )) {
        ModuleFunctionPreparation::Ready(proof) => proof,
        ModuleFunctionPreparation::Pending(demand) => {
            panic!("actual module admission pending: {demand:?}")
        }
    }
}
fn no_executable_work(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
                | QueryKey::EnteredType(_)
                | QueryKey::PreparedTypePlan(_)
                | QueryKey::TypeSchema(_)
        )),
        "module admission forced an executable body or explicit generic Type"
    );
}
#[test]
fn official_auto_module_admission_retains_actual_source_guard_without_second_lookup_all_origins() {
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..3 {
            let text = if mode == 0 {
                AUTO.to_owned()
            } else {
                emit(AUTO, edition, mode == 2)
            };
            let (mut db, parent) = world(&text, edition, false, None);
            let sites = calls(&parent);
            assert_eq!(sites.len(), 1);
            let start = db.trace.events.len();
            let proof = admit(&mut db, &parent, sites[0]);
            assert_eq!(proof.canonical.definition().declared_name, "auto");
            assert!(Arc::ptr_eq(proof.parent(), &parent));
            assert_eq!(proof.canonical._syntax.is_some(), mode != 0);
            let child = proof.canonical.body.as_ref().unwrap().key();
            assert!(db.memo(&QueryKey::BodyStructure(child)).is_none());
            no_executable_work(&db, start);
            let projected_at = db.trace.events.len();
            let callee = proof.callable_shared(&mut db).unwrap();
            assert_eq!(callee.identity(), proof.canonical.identity);
            assert_eq!(callee.body(), proof.canonical.body.as_ref());
            assert!(callee.ordinary_local_contract().is_none());
            assert!(std::ptr::eq(
                callee.ordinary_module_contract().unwrap(),
                proof.as_ref()
            ));
            for _ in 0..3 {
                proof.revalidate(&db).unwrap();
                callee.revalidate(&mut db).unwrap();
            }
            assert_eq!(
                db.trace.events.len(),
                projected_at,
                "retained projection/currentness reentered source planning"
            );
            db.begin_expansion_epoch(2).unwrap();
            let revoked_at = db.trace.events.len();
            let error = proof.revalidate(&db).unwrap_err();
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(error.message, "prepared operation input changed");
            let error = proof.callable_shared(&mut db).unwrap_err();
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(error.message, "prepared operation input changed");
            assert_eq!(db.trace.events.len(), revoked_at);
        }
    }
}
#[test]
fn official_auto_retained_module_target_uses_real_lease_and_original_caller() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, parent) = world(AUTO, edition, false, Some("auto"));
        let start = db.trace.events.len();
        let proof = admit(&mut db, &parent, calls(&parent)[0]);
        assert!(proof.canonical._retained.is_some());
        assert!(matches!(
            proof.canonical._syntax.as_deref(),
            Some(InputValue::RetainedSyntax(_))
        ));
        assert_ne!(
            proof.canonical.source(),
            &parent.context().source().parsed().key()
        );
        let callee = proof.callable_shared(&mut db).unwrap();
        assert_eq!(callee.identity(), proof.canonical.identity);
        no_executable_work(&db, start);
    }
}

#[test]
fn canonical_module_projection_rejects_rebound_source_header_context_and_namespace() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, parent) = world(AUTO, edition, false, None);
        let proof = admit(&mut db, &parent, calls(&parent)[0]);
        let actual = &proof.canonical;
        for mutation in 0..4 {
            let mut selected = proof.declaration().definition().clone();
            let mut source = actual.source().clone();
            let mut header = actual.header().clone();
            let mut context = actual.context().clone();
            match mutation {
                0 => source.content = Digest::of_bytes(b"not the selected target source"),
                1 => header.tokens[0].text.push_str("_wrong"),
                2 => context.range.start = context.range.start.checked_add(1).unwrap(),
                3 => selected.namespace = Namespace::Type,
                _ => unreachable!(),
            }
            let start = db.trace.events.len();
            match canonical_module_declaration(
                &mut db, &parent, &selected, &source, &header, &context,
            ) {
                QueryPoll::Ready(Err(error)) => {
                    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
                    assert_eq!(
                        error.message,
                        if mutation == 3 {
                            "selected module source/namespace/owner differs"
                        } else {
                            "selected module actual declaration rows differ"
                        }
                    );
                }
                _ => panic!("rebound module source facts acquired canonical authority"),
            }
            no_executable_work(&db, start);
            proof.revalidate(&db).unwrap();
        }
    }
}
#[test]
fn official_nested_statement_macro_retained_caller_selects_original_module_header_only() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, parent) = world(NESTED, edition, true, None);
        assert!(
            parent
                .context()
                .source()
                .retained_inputs()
                .any(|input| matches!(input.as_ref(),
            InputValue::RetainedSyntax(input) if !input.prepared().outputs().is_empty()))
        );
        let sites = calls(&parent);
        assert_eq!(sites.len(), 3);
        let start = db.trace.events.len();
        for site in sites {
            let proof = admit(&mut db, &parent, site);
            assert_eq!(
                proof.canonical.definition().declared_name,
                "static_assert_exists"
            );
            assert!(proof.canonical._retained.is_none());
            assert!(proof.canonical._syntax.is_none());
            assert_ne!(
                proof.canonical.source(),
                &parent.context().source().parsed().key()
            );
            assert_eq!(proof.signature.generics.len(), 1);
            assert!(proof.signature.parameters.is_empty());
            let child = proof.canonical.body.as_ref().unwrap().key();
            assert!(db.memo(&QueryKey::BodyStructure(child)).is_none());
        }
        no_executable_work(&db, start);
        assert_eq!(
            db.trace.events[start..]
                .iter()
                .filter(|event| {
                    event.action == Action::ProviderRun && event.key.kind() == "FunctionSignature"
                })
                .count(),
            1,
            "one actual module signature must serve all three base selections"
        );
    }
}

#[path = "module_target_publication_tests.rs"]
mod module_target_publication_tests;
