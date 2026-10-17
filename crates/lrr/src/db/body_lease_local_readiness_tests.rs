//! Actual retained local source lease controls; no frame execution or readiness
//! permission is manufactured by the test. Full stock execution is separate.
use super::*;
use crate::{
    db::{provider::SyntaxProvider, trace::Action},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        callables::{CallSelection, EnteredLocalCall, prepare_entered_local_function},
        compile_input_catalog::CompileInputCatalog,
        source_body_context,
        source_locals::PreparedSourceLocalPlan,
    },
    ir::pc::{NodeId, Phase},
    source::manifest::Namespace,
    syntax::body_index::{ExpressionKind, NodeRole},
};
const SHADOW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/type-param-local-var-shadowing.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("local execution lease source authority: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("local execution lease source demand: {input:?}"),
    }
}
fn world(edition: u16) -> (Database, Arc<PreparedSourceLocalPlan>, BodyKey) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(SHADOW, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"retained local execution lease authority"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"retained local execution lease compiler"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(&mut db).unwrap();
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("actual complete stock discovery incomplete: {diagnostic:?}; {blocked:?}"),
            _ => panic!("actual complete stock discovery requires an unsupported producer stage"),
        }
    }
    let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, "main")) else {
        panic!("actual source main absent")
    };
    // Same actual CLI selected-body preparation that exposed the regression.
    let main =
        crate::cli::retained_entry::prepare(&mut db, &discovery, &source, "main", main).unwrap();
    let parent =
        ready(db.prepare_source_local_plan(&discovery, &[&source], &main.body().unwrap().key()));
    let index = parent.context().source().index();
    let mut calls = index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call));
    let call = calls
        .next()
        .expect("unchanged inner block has actual f call");
    assert!(calls.next().is_none());
    let site = index.pc(call.id, Phase::Entry).unwrap();
    let EnteredLocalCall::Local(proof) = ready(prepare_entered_local_function(&mut db, &site))
    else {
        panic!("actual entered f selection must be Local")
    };
    let child = proof.item().body.as_ref().unwrap().key();
    db.begin_run().unwrap();
    db.publish_ordinary_local_function(&proof).unwrap();
    let callee = proof.callable(&mut db).unwrap();
    let structure = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(child.clone())));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("actual selected child index absent")
    };
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let admitted = Arc::new(ready(source_body_context::capture_local(
        &mut db,
        Arc::clone(&parent),
        &callee,
        index,
        &entry,
    )));
    assert!(Arc::ptr_eq(admitted.local_parent().unwrap(), &parent));
    db.finish_run().unwrap();
    let macros = Arc::clone(parent.environment().unwrap().macros());
    ready(db.publish_body_environment(admitted, macros));
    (db, parent, child)
}
#[test]
fn actual_stock_retained_child_uses_its_opaque_owning_environment_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, parent, child) = world(edition);
        let parse = parent.context().source().parsed().key();
        let retained = db.input(&InputKey::RetainedSyntax(parse.content)).unwrap();
        let InputValue::RetainedSyntax(retained) = retained.as_ref() else {
            panic!("actual retained parent source absent")
        };
        assert!(
            retained
                .prepared()
                .permits_body(parent.context().source().body().def_key)
        );
        assert!(
            !retained.prepared().permits_body(child.definition),
            "parent permit must remain body scoped"
        );
        let trace = db.trace.events.len();
        let lease = ready(db.pin_guest_body(&child));
        assert_eq!(lease.body(), &child);
        assert!(matches!(
            lease.0._source.as_ref(),
            InputValue::BodyEnvironment(_)
        ));
        ready(lease.require_current(&mut db));
        assert!(
            db.trace.events[trace..]
                .iter()
                .all(|event| event.action != Action::ProviderRun),
            "lease currentness must consume retained source proof without preparing syntax"
        );
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(event.key, QueryKey::Thunk(_))),
            "source lease is not guest execution"
        );
        drop(lease);
    }
}
#[test]
fn missing_wrong_or_revoked_local_environment_never_falls_back_to_parent_retained_source() {
    let (mut db, parent, child) = world(2024);
    let key = InputKey::BodyEnvironment(child.clone());
    let actual = db.inputs.remove(&key).unwrap();
    assert!(matches!(
        db.pin_guest_body(&child),
        QueryPoll::Ready(Err(_))
    ));
    let value = InputValue::BodyEnvironment(Arc::clone(parent.environment().unwrap()));
    db.inputs.insert(
        key.clone(),
        InputRecord {
            fingerprint: value.fingerprint(),
            value: Arc::new(value),
        },
    );
    assert!(matches!(
        db.pin_guest_body(&child),
        QueryPoll::Ready(Err(_))
    ));
    db.inputs.insert(key.clone(), actual);
    let lease = ready(db.pin_guest_body(&child));
    let actual = db.inputs.remove(&key).unwrap();
    assert!(matches!(
        lease.require_current(&mut db),
        QueryPoll::Ready(Err(_))
    ));
    db.inputs.insert(key, actual);
    ready(lease.require_current(&mut db));
    let owner_key = InputKey::LocalDeclarationOwner(child.definition);
    let owner = db.inputs.remove(&owner_key).unwrap();
    assert!(matches!(
        lease.require_current(&mut db),
        QueryPoll::Ready(Err(_))
    ));
    // A missing positive owner cannot turn the same parent retained parse into
    // an independent permit for this foreign body at a new lease boundary.
    assert!(matches!(
        db.pin_guest_body(&child),
        QueryPoll::Ready(Err(_))
    ));
    db.inputs.insert(owner_key, owner);
    ready(lease.require_current(&mut db));
    drop(lease);
}
#[test]
fn actual_lease_keeps_local_source_charged_and_refuses_changed_rows() {
    let (mut db, _parent, child) = world(2024);
    let lease = ready(db.pin_guest_body(&child));
    assert_eq!(
        db.require_body_unowned(child.definition).unwrap_err().code,
        "LRR-E-BODY-ACTIVE"
    );
    let key = InputKey::DefinitionSource(child.definition);
    let source = db.inputs.remove(&key).unwrap();
    assert!(matches!(
        lease.require_current(&mut db),
        QueryPoll::Ready(Err(_))
    ));
    db.inputs.insert(key, source);
    ready(lease.require_current(&mut db));
    assert!(db.cache_usage().unwrap().memo_bytes > 0);
    drop(lease);
    db.require_body_unowned(child.definition).unwrap();
}
