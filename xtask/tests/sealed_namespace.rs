use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> (PreparedSourceView, PreparedSourceInputs) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
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
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    (view, inputs)
}

fn discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}

fn finish(w: &mut SourceDiscovery) {
    loop {
        match w.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. }
            | SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                panic!("fixture failed to complete discovery: {diagnostic:?} {blocked:?}")
            }
        }
    }
}
fn query_key(
    w: &SourceDiscovery,
    name: &str,
    scope: lrr::syntax::outline::ScopeKey,
    namespace: lrr::source::manifest::Namespace,
) -> lrr::db::key::QueryKey {
    lrr::db::key::QueryKey::SealedName(Box::new(lrr::db::key::SealedNameKey {
        seal: w.key().clone(),
        bucket: lrr::db::key::BucketKey {
            scope,
            namespace,
            name: name.into(),
        },
    }))
}
fn lookup(
    db: &mut lrr::db::memo::Database,
    key: lrr::db::key::QueryKey,
) -> lrr::frontend::names::NameLookup {
    let lrr::db::poll::QueryPoll::Ready(Ok(outcome)) =
        db.fetch(&lrr::db::provider::SyntaxProvider, key)
    else {
        panic!("certified name query not ready")
    };
    let lrr::db::value::QueryValue::SealedName { lookup, .. } = outcome.value.as_ref() else {
        panic!("wrong query value")
    };
    lookup.clone()
}
#[test]
fn sealed_namespace_queries_use_real_generated_imports_and_certify_absence() {
    use lrr::db::{
        dependency::DependencyStamp,
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    };
    let (v, inputs) =
        source("use made as selected;macro_rules! make{()=>{pub fn made(){7_i64;}}}make!();");
    let mut w = discover(&v);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let missing = query_key(
        &w,
        "absent",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
    );
    assert!(matches!(
        db.fetch(&SyntaxProvider, missing.clone()),
        QueryPoll::NeedInput(InputRequest::Discover { .. })
    ));
    assert!(db.memo(&missing).is_none());
    finish(&mut w);
    w.publish(&mut db).unwrap();
    let key = query_key(
        &w,
        "selected",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
    );
    assert_eq!(lookup(&mut db, key), w.lookup("selected").unwrap());
    assert_eq!(
        lookup(&mut db, missing.clone()),
        lrr::frontend::names::NameLookup::Absent
    );
    assert!(db.memo(&missing).unwrap().dependencies.iter().any(
        |d| matches!(d,DependencyStamp::Query{key:QueryKey::DiscoverySeal(key),..} if key==w.key())
    ));
}
#[test]
fn sealed_namespace_preserves_ambiguity_and_rejects_foreign_domain() {
    let (v, inputs) = source("pub fn duplicate(){}pub fn duplicate(){}");
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let key = query_key(
        &w,
        "duplicate",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
    );
    let lrr::frontend::names::NameLookup::Ambiguous(values) = lookup(&mut db, key) else {
        panic!("ambiguity lost")
    };
    assert_eq!(values.len(), 2);
    let wrong = query_key(
        &w,
        "duplicate",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Type,
    );
    let lrr::db::poll::QueryPoll::Ready(Err(error)) =
        db.fetch(&lrr::db::provider::SyntaxProvider, wrong)
    else {
        panic!("wrong namespace borrowed seal")
    };
    assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
}
#[test]
fn certified_missing_name_invalidates_when_source_snapshot_is_replaced() {
    let (v, inputs) = source("pub fn ordinary(){}");
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let key = query_key(
        &w,
        "new_name",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
    );
    assert_eq!(
        lookup(&mut db, key.clone()),
        lrr::frontend::names::NameLookup::Absent
    );
    let (next, changed) = source("pub fn new_name(){}");
    changed.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, key.clone()),
        lrr::db::poll::QueryPoll::NeedInput(_)
    ));
    let mut next_work = discover(&next);
    finish(&mut next_work);
    next_work.publish(&mut db).unwrap();
    assert!(matches!(
        lookup(&mut db, key),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}
#[test]
fn sealed_type_namespace_retains_module_and_type_outcomes() {
    let (v, inputs) = source("pub struct Item;pub mod inner{}");
    let scope = v.outline().root_scope.clone();
    let mut w = SourceDiscovery::new(
        vec![v.share()],
        scope.clone(),
        lrr::source::manifest::Namespace::Type,
        Digest([245; 32]),
        1,
    )
    .unwrap();
    finish(&mut w);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    assert_eq!(
        lookup(
            &mut db,
            query_key(
                &w,
                "Item",
                scope.clone(),
                lrr::source::manifest::Namespace::Type
            )
        ),
        w.lookup("Item").unwrap()
    );
    assert_eq!(
        lookup(
            &mut db,
            query_key(&w, "inner", scope, lrr::source::manifest::Namespace::Type)
        ),
        w.lookup("inner").unwrap()
    );
}

#[test]
fn certified_type_namespace_preserves_imported_module_alias() {
    let (v, inputs) = source("pub mod inner{}use inner as alias;");
    let scope = v.outline().root_scope.clone();
    let mut w = SourceDiscovery::new(
        vec![v.share()],
        scope.clone(),
        lrr::source::manifest::Namespace::Type,
        Digest([245; 32]),
        1,
    )
    .unwrap();
    finish(&mut w);
    let expected = w.lookup("alias").unwrap();
    assert_ne!(expected, lrr::frontend::names::NameLookup::Absent);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    assert_eq!(
        lookup(
            &mut db,
            query_key(&w, "alias", scope, lrr::source::manifest::Namespace::Type)
        ),
        expected
    );
}
#[test]
fn unfinished_macro_namespace_routing_cannot_certify_missing_macro() {
    let (v, inputs) = source("macro_rules! known{()=>{}}");
    let scope = v.outline().root_scope.clone();
    let mut w = SourceDiscovery::new(
        vec![v.share()],
        scope.clone(),
        lrr::source::manifest::Namespace::Macro,
        Digest([245; 32]),
        1,
    )
    .unwrap();
    finish(&mut w);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let key = query_key(&w, "known", scope, lrr::source::manifest::Namespace::Macro);
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, key.clone()),
        lrr::db::poll::QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { .. })
    ));
    assert!(db.memo(&key).is_none());
}

fn macro_discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Macro,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}

#[test]
fn completed_macro_namespace_certifies_members_and_absence() {
    use lrr::db::{dependency::DependencyStamp, key::QueryKey};
    let (v, inputs) = source("macro_rules! known{()=>{}}");
    let mut work = macro_discover(&v);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    let missing = query_key(
        &work,
        "missing",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Macro,
    );
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, missing.clone()),
        lrr::db::poll::QueryPoll::NeedInput(_)
    ));
    assert!(db.memo(&missing).is_none());
    finish(&mut work);
    work.publish(&mut db).unwrap();
    let known = query_key(
        &work,
        "known",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Macro,
    );
    assert!(matches!(
        lookup(&mut db, known),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    assert_eq!(
        lookup(&mut db, missing.clone()),
        lrr::frontend::names::NameLookup::Absent
    );
    assert!(db.memo(&missing).unwrap().dependencies.iter().any(|d| matches!(d, DependencyStamp::Query {key:QueryKey::DiscoverySeal(key),..} if key == work.key())));
}

#[test]
fn completed_macro_namespace_preserves_generated_export_and_import_identity() {
    let (v, inputs) = source(
        "macro_rules! outer{()=>{#[macro_export]macro_rules! seed{()=>{}}use crate::seed as picked;}}outer!();",
    );
    let mut work = macro_discover(&v);
    finish(&mut work);
    let expected = work.lookup("picked").unwrap();
    let lrr::frontend::names::NameLookup::Defined(definition) = &expected else {
        panic!("macro alias absent")
    };
    let records = work.sealed_expansions().unwrap().collect::<Vec<_>>();
    let generated = lrr::expand::generated_items::GeneratedItems::admit(
        records[0].key(),
        records[0].clone(),
        1,
    )
    .unwrap();
    assert!(
        generated
            .items()
            .iter()
            .any(|h| h.key.fingerprint() == definition.identity)
    );
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    for name in ["seed", "picked"] {
        assert_eq!(
            lookup(
                &mut db,
                query_key(
                    &work,
                    name,
                    v.outline().root_scope.clone(),
                    lrr::source::manifest::Namespace::Macro
                )
            ),
            expected
        );
    }
    let absent = query_key(
        &work,
        "not_generated",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Macro,
    );
    assert_eq!(
        lookup(&mut db, absent.clone()),
        lrr::frontend::names::NameLookup::Absent
    );
    assert!(db.memo(&absent).unwrap().dependencies.iter().any(|d| matches!(d, lrr::db::dependency::DependencyStamp::Query {key:lrr::db::key::QueryKey::DiscoverySeal(key),..} if key == work.key())));
    let seal_key = lrr::db::key::QueryKey::DiscoverySeal(work.key().clone());
    for record in records {
        assert!(db.memo(&seal_key).unwrap().dependencies.iter().any(|d| matches!(d, lrr::db::dependency::DependencyStamp::Input {key:lrr::db::dependency::InputKey::ExpansionRecord(id),..} if *id == record.key().fingerprint())));
    }
}

#[test]
fn completed_macro_namespace_resolves_original_textual_import_without_invocation() {
    let (v, inputs) = source("macro_rules! known{()=>{}}use known as picked;");
    let mut work = macro_discover(&v);
    finish(&mut work);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let expected = work.lookup("known").unwrap();
    assert!(matches!(
        expected,
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    assert_eq!(
        lookup(
            &mut db,
            query_key(
                &work,
                "picked",
                v.outline().root_scope.clone(),
                lrr::source::manifest::Namespace::Macro
            )
        ),
        expected
    );
}

#[test]
fn certified_macro_absence_invalidates_after_source_replacement() {
    let (v, inputs) = source("macro_rules! old{()=>{}}");
    let mut work = macro_discover(&v);
    finish(&mut work);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = query_key(
        &work,
        "added",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Macro,
    );
    assert_eq!(
        lookup(&mut db, key.clone()),
        lrr::frontend::names::NameLookup::Absent
    );
    let (next, replacement) = source("macro_rules! added{()=>{}}");
    replacement.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, key.clone()),
        lrr::db::poll::QueryPoll::NeedInput(_)
    ));
    let mut next_work = macro_discover(&next);
    finish(&mut next_work);
    next_work.publish(&mut db).unwrap();
    assert!(matches!(
        lookup(&mut db, key),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}

#[test]
fn unresolved_macro_producer_cannot_certify_a_missing_bucket() {
    let (v, inputs) = source("missing!();");
    let mut work = macro_discover(&v);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
    let key = query_key(
        &work,
        "absent",
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Macro,
    );
    assert!(matches!(
        db.fetch(&lrr::db::provider::SyntaxProvider, key.clone()),
        lrr::db::poll::QueryPoll::NeedInput(_)
    ));
    assert!(db.memo(&key).is_none());
}

#[test]
fn certified_macro_membership_preserves_duplicate_candidate_identity() {
    let (v, inputs) = source("macro_rules! same{()=>{}}macro_rules! same{()=>{}}");
    let mut work = macro_discover(&v);
    finish(&mut work);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let lrr::frontend::names::NameLookup::Ambiguous(definitions) = lookup(
        &mut db,
        query_key(
            &work,
            "same",
            v.outline().root_scope.clone(),
            lrr::source::manifest::Namespace::Macro,
        ),
    ) else {
        panic!("candidate membership arbitrarily selected")
    };
    assert_eq!(definitions.len(), 2);
    assert_ne!(definitions[0].identity, definitions[1].identity);
}

#[test]
fn certified_macro_names_preserve_raw_spelling_and_cfg_membership() {
    let (v, inputs) =
        source("macro_rules! r#async{()=>{}}#[cfg(any())]macro_rules! hidden{()=>{}}");
    let mut work = macro_discover(&v);
    finish(&mut work);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = |name| {
        query_key(
            &work,
            name,
            v.outline().root_scope.clone(),
            lrr::source::manifest::Namespace::Macro,
        )
    };
    let expected = lookup(&mut db, key("r#async"));
    assert!(matches!(
        expected,
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    assert_eq!(lookup(&mut db, key("async")), expected);
    assert_eq!(
        lookup(&mut db, key("hidden")),
        lrr::frontend::names::NameLookup::Absent
    );
    let mut foreign = v.outline().root_scope.clone();
    foreign.module_path.push("foreign".into());
    let wrong = query_key(
        &work,
        "async",
        foreign,
        lrr::source::manifest::Namespace::Macro,
    );
    let lrr::db::poll::QueryPoll::Ready(Err(error)) =
        db.fetch(&lrr::db::provider::SyntaxProvider, wrong)
    else {
        panic!("macro certificate borrowed by foreign scope")
    };
    assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
}
