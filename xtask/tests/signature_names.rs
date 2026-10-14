use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        names::{NameLookup, NameNamespaces},
        signature_names::{SignatureName, SignatureNames},
    },
    ir::op::PrimitiveType,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::ItemKind,
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
fn fetch_signature(
    db: &mut Database,
    definition: Digest,
) -> Box<lrr::syntax::signatures::FunctionSignature> {
    let QueryPoll::Ready(Ok(value)) =
        db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition))
    else {
        panic!("signature demand failed")
    };
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("signature result mismatch")
    };
    signature.clone()
}
fn live(view: &PreparedSourceView) -> Digest {
    view.outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "live")
        .unwrap()
        .def_key
        .fingerprint()
}
#[test]
fn signature_name_resolution_uses_source_identity_before_primitive_fallback() {
    for (text, shadowed) in [
        ("fn live<T: Missing>(x:i64,y:T){}", false),
        ("struct i64; fn live<T: Missing>(x:i64,y:T){}", true),
    ] {
        let (inputs, view) = source(text);
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let signature = fetch_signature(&mut db, live(&view));
        let names = SignatureNames::new(&graph, &view, &signature).unwrap();
        if shadowed {
            let expected = view
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.item_kind == ItemKind::Struct)
                .unwrap()
                .def_key
                .fingerprint();
            assert!(
                matches!(names.unqualified("i64",None).unwrap(),SignatureName::Source(d) if d.identity==expected)
            );
        } else {
            assert_eq!(
                names.unqualified("i64", None).unwrap(),
                SignatureName::Primitive(PrimitiveType::I64)
            );
        }
        assert!(
            matches!(names.unqualified("T",None).unwrap(),SignatureName::Parameter(p) if p.definition==signature.definition&&p.ordinal==0)
        );
        assert_eq!(
            names.unqualified("Absent", None).unwrap(),
            SignatureName::Unresolved(NameLookup::Absent)
        );
        assert!(
            db.trace
                .events
                .iter()
                .all(|e| !matches!(e.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
    }
}
#[test]
fn signature_name_resolution_preserves_aliases_ambiguity_and_pending_producers() {
    let (inputs, view) = source("type Word=u64; use Word as Number; fn live(x:Number){}");
    let mut graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    graph.resolve_imports().unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let signature = fetch_signature(&mut db, live(&view));
    let names = SignatureNames::new(&graph, &view, &signature).unwrap();
    let expected = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.item_kind == ItemKind::TypeAlias)
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(names.unqualified("Number",None).unwrap(),SignatureName::Source(d) if d.identity==expected)
    );
    let path = NameNamespaces::path(signature.scope.crate_key, &["Word"], 2024).unwrap();
    assert!(
        matches!(names.qualified(&path).unwrap(),SignatureName::Source(d) if d.identity==expected)
    );
    for (text, pending) in [
        ("struct i64;struct i64;fn live(x:i64){}", false),
        ("make!();fn live(x:i64){}", true),
    ] {
        let (inputs, view) = source(text);
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let signature = fetch_signature(&mut db, live(&view));
        let names = SignatureNames::new(&graph, &view, &signature).unwrap();
        let found = names.unqualified("i64", None).unwrap();
        if pending {
            assert!(matches!(
                found,
                SignatureName::Unresolved(NameLookup::NeedInput(_))
            ))
        } else {
            assert!(
                matches!(found,SignatureName::Unresolved(NameLookup::Ambiguous(ds)) if ds.len()==2)
            )
        }
    }
}
#[test]
fn exact_declaration_context_distinguishes_bodyless_trait_signatures_and_rejects_stale_graph() {
    let (inputs, view) = source("trait A{fn same(x:i64);}trait B{fn same(x:i64);}fn live(){}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let methods: Vec<_> = view
        .outline()
        .items
        .iter()
        .filter(|i| i.def_key.declared_name == "same")
        .collect();
    assert_eq!(methods.len(), 2);
    let a = fetch_signature(&mut db, methods[0].def_key.fingerprint());
    let b = fetch_signature(&mut db, methods[1].def_key.fingerprint());
    assert_ne!(a.definition, b.definition);
    assert_ne!(a.scope.owner, b.scope.owner);
    assert!(matches!(
        SignatureNames::new(&graph, &view, &a)
            .unwrap()
            .unqualified("Self", None)
            .unwrap(),
        SignatureName::NeedOwnerContext(_)
    ));

    for signature in [&a, &b] {
        assert!(matches!(
            SignatureNames::new(&graph, &view, signature)
                .unwrap()
                .unqualified("i64", None)
                .unwrap(),
            SignatureName::NeedOwnerContext(_)
        ));
    }
    assert!(
        db.trace
            .events
            .iter()
            .all(|e| !matches!(e.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
    let signature = fetch_signature(&mut db, live(&view));
    let (_, changed) =
        source("trait A{fn same(x:i64);}trait B{fn same(x:i64);}fn live(){missing();}");
    assert_eq!(
        SignatureNames::new(&graph, &changed, &signature)
            .err()
            .unwrap()
            .code,
        "LRR-E-MACRO-SCOPE"
    );
}
