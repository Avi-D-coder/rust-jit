use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
    },
    frontend::{
        callables::{CallResolver, CallSelection, SourceCallable},
        names::{NameLookup, NameNamespaces},
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/main.rs").unwrap();
    let crate_key = Digest([151; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source missing")
    };
    (inputs, view)
}

fn id(view: &PreparedSourceView, name: &str) -> Digest {
    view.outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap()
        .def_key
        .fingerprint()
}
fn selected(poll: QueryPoll<CallSelection>) -> SourceCallable {
    match poll {
        QueryPoll::Ready(Ok(CallSelection::Source(callable))) => *callable,
        _ => panic!("source callable not selected"),
    }
}
#[test]
fn direct_call_selection_captures_only_reached_signature_without_bound_or_body_entry() {
    let (inputs, view) = source(
        "fn caller(){} fn chosen<T: Unknown>(x:T)->T{missing();} fn cold<T,T>(x:Missing){absent();}",
    );
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let target = selected(resolver.module_function(&mut db, id(&view, "caller"), "chosen"));
    assert_eq!(target.definition().fingerprint(), id(&view, "chosen"));
    assert_eq!(target.signature().generics[0].name, "T");
    assert!(target.body().is_some());
    target.revalidate(&mut db).unwrap();
    assert!(db.trace.events.iter().all(|e| !matches!(
        e.key,
        QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::Constant(_)
    )));
    let cold = id(&view, "cold");
    assert!(db.trace.events.iter().all(
        |e| !matches!(e.key,QueryKey::Signature(def)|QueryKey::FunctionSignature(def)if def==cold)
    ));
}
#[test]
fn call_selection_imports_preserve_exact_identity_and_other_values_stay_deferred() {
    let (inputs, view) = source(
        "mod lib{pub fn chosen<T: Unknown>(x:T)->T{x}}use lib::chosen as invoke; const VALUE:fn()=missing; fn caller(){}",
    );
    let mut graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    graph.resolve_imports().unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let caller = id(&view, "caller");
    let first = selected(resolver.module_function(&mut db, caller, "invoke"));
    let path =
        NameNamespaces::path(first.definition().crate_key, &["lib", "chosen"], 2024).unwrap();
    let second = selected(resolver.qualified(&mut db, caller, &path));
    assert_eq!(first.definition(), second.definition());
    assert_eq!(first.signature().definition, id(&view, "chosen"));
    assert_eq!(first.signature().scope.module_path, ["lib"]);
    assert!(
        matches!(resolver.module_function(&mut db,caller,"VALUE"),QueryPoll::Ready(Ok(CallSelection::NeedValue(def)))if def.fingerprint()==id(&view,"VALUE"))
    );
}
#[test]
fn missing_ambiguous_pending_and_stale_call_contexts_do_not_select_another_target() {
    for (text, kind) in [
        ("fn caller(){}", 0),
        ("fn caller(){}fn chosen(){}fn chosen(){}", 1),
        ("make!();fn caller(){}fn chosen(){}", 2),
    ] {
        let (inputs, view) = source(text);
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let sources = [&view];
        let resolver = CallResolver::new(&graph, &sources).unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let result = resolver.module_function(&mut db, id(&view, "caller"), "chosen");
        match kind {
            0 => assert!(
                matches!(result,QueryPoll::Ready(Err(error))if error.code=="LRR-E-NAME-MISSING")
            ),
            1 => assert!(
                matches!(result,QueryPoll::Ready(Ok(CallSelection::Unresolved(NameLookup::Ambiguous(ds))))if ds.len()==2)
            ),
            2 => assert!(matches!(
                result,
                QueryPoll::Ready(Ok(CallSelection::NeedProducer(_)))
            )),
            _ => panic!("invalid test case"),
        };
        assert!(db.trace.events.iter().all(|e| !matches!(
            e.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
        )));
    }
    let (inputs, view) = source("fn caller(){}fn chosen(){1;}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let target = selected(resolver.module_function(&mut db, id(&view, "caller"), "chosen"));
    let (changed, _) = source("fn caller(){}fn chosen(){2;}");
    changed.publish(&mut db).unwrap();
    assert_eq!(
        target.revalidate(&mut db).unwrap_err().code,
        "LRR-E-CALL-CONTEXT"
    );
}
