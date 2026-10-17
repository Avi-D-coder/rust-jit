//! Source-component acceptance; unchanged originals never imply full-main passes.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::constants::{ConstLimits, evaluate},
    source::{
        changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const MODULE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/module-const-array-size-13763.rs"
);
const NESTED: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-nested.rs");
const LOCALS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/locals-in-const-fn.rs"
);
const SHADOW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/shadow-const-param.rs"
);
fn world(
    text: &str,
    name: &str,
    edition: u16,
    seal_type: bool,
) -> (Database, PreparedSourceView, Digest) {
    let unit = Digest::of_bytes(b"LRR current constant source acceptance");
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("constant-original.rs").unwrap();
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
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap();
    let definition = item.def_key.fingerprint();
    let scope = item.scope.clone();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    for namespace in [Namespace::Value, Namespace::Type] {
        if namespace == Namespace::Type && !seal_type {
            continue;
        }
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"LRR constant source current world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"LRR constant declarative"),
            Digest::of_bytes(b"LRR constant compiler"),
            Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                    .unwrap(),
            ),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                    "actual discovery incomplete: {} {}",
                    diagnostic.code, diagnostic.message
                ),
                _ => panic!("unestablished constant source authority"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    (db, view, definition)
}
fn admit(
    db: &Database,
    view: &PreparedSourceView,
    definition: Digest,
) -> Result<ConstantSourceAdmission, Diagnostic> {
    ConstantSourceAdmission::capture_current(
        db,
        view,
        definition,
        ConstantRequest::Constant,
        usize::MAX,
    )
}
#[test]
fn unchanged_module_bits_has_exact_owning_module_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(MODULE, "BITS", edition, true);
        let certificate = admit(&db, &view, definition).unwrap();
        assert_eq!(
            certificate.plan().owner_context().scope.module_path,
            vec!["u8"]
        );
        assert_eq!(certificate.plan().owner_definition(), definition);
        assert_eq!(certificate.plan().profile().edition, edition);
        let evaluated = evaluate(certificate.plan(), ConstLimits::default()).unwrap();
        assert_eq!(evaluated.value().scalar().unwrap().payload, 8);
        certificate.revalidate(&db, &view).unwrap();
        // Qualified NUM routing/main construction remain a separate runtime frontier.
        let (db, view, definition) = world(MODULE, "NUM", edition, true);
        let certificate = admit(&db, &view, definition).unwrap();
        assert!(evaluate(certificate.plan(), ConstLimits::default()).is_err());
    }
}
#[test]
fn unchanged_nested_const_calls_preserve_definition_and_arguments_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(NESTED, "X", edition, true);
        let before = db.trace.events.len();
        let certificate = admit(&db, &view, definition).unwrap();
        assert_eq!(
            evaluate(certificate.plan(), ConstLimits::default())
                .unwrap()
                .value()
                .scalar()
                .unwrap()
                .payload,
            22
        );
        certificate.revalidate(&db, &view).unwrap();
        assert_eq!(db.trace.events.len(), before);
        assert!(
            certificate
                .dependencies()
                .contains_key(&InputKey::DefinitionBody(definition))
        );
        assert!(certificate.metadata_bytes() <= usize::MAX);
    }
}
#[test]
fn unchanged_const_fn_local_alias_and_mutation_frontier() {
    let (db, view, definition) = world(LOCALS, "foo", 2024, true);
    let argument = ConstValue::from_scalar(
        crate::runtime::value::ValueRepr::unsigned(crate::runtime::shape::TypeId(13), 42).unwrap(),
    )
    .unwrap();
    let certificate = ConstantSourceAdmission::capture_current(
        &db,
        &view,
        definition,
        ConstantRequest::ConstCall(vec![argument.clone()]),
        usize::MAX,
    )
    .unwrap();
    assert_eq!(
        evaluate(certificate.plan(), ConstLimits::default())
            .unwrap()
            .value()
            .scalar()
            .unwrap()
            .payload,
        42
    );
    let (db, view, definition) = world(LOCALS, "bar", 2024, true);
    // The original mutable binding is not silently treated as an immutable parameter.
    let certificate = ConstantSourceAdmission::capture_current(
        &db,
        &view,
        definition,
        ConstantRequest::ConstCall(vec![argument]),
        usize::MAX,
    );
    assert!(evaluate(certificate.unwrap().plan(), ConstLimits::default()).is_err());
}
#[test]
fn absence_of_type_seal_is_a_precise_prerequisite() {
    let (db, view, definition) = world(NESTED, "X", 2024, false);
    assert!(admit(&db, &view, definition).is_err());
}
#[test]
fn primitive_looking_type_alias_is_never_descriptor_recovery() {
    let text = "type i32=bool;const X:i32=true;fn main(){}";
    let (db, view, definition) = world(text, "X", 2024, true);
    assert!(admit(&db, &view, definition).is_err());
}
#[test]
fn import_or_producer_cannot_grant_primitive_identity() {
    let (db, view, definition) = world(
        "mod m{pub type i32=bool;}use m::i32;const X:i32=true;fn main(){}",
        "X",
        2024,
        true,
    );
    assert!(admit(&db, &view, definition).is_err());
}
#[test]
fn stale_derived_epoch_refuses_without_source_revision_change() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let certificate = admit(&db, &view, definition).unwrap();
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), revision);
    assert!(certificate.revalidate(&db, &view).is_err());
}
#[test]
fn copied_wrong_source_and_mutable_public_fact_pairs_refuse() {
    let (mut db, view, definition) = world(NESTED, "X", 2024, true);
    let certificate = admit(&db, &view, definition).unwrap();
    let (_, other, _) = world("const X:u32=22;fn main(){}", "X", 2024, true);
    assert!(certificate.revalidate(&db, &other).is_err());
    let mut facts = view.prepared_inputs().clone();
    facts.remove(&InputKey::DefinitionBody(definition));
    db.publish_inputs(facts).unwrap();
    assert!(admit(&db, &view, definition).is_err());
    assert!(certificate.revalidate(&db, &view).is_err());
}
#[test]
fn no_seal_and_metadata_refusal_publish_nothing() {
    let (db, view, definition) = world(NESTED, "X", 2024, true);
    let before = db.trace.events.len();
    assert!(
        ConstantSourceAdmission::capture_current(
            &db,
            &view,
            definition,
            ConstantRequest::Constant,
            1
        )
        .is_err()
    );
    assert_eq!(db.trace.events.len(), before);
    assert!(
        ConstantSourceAdmission::capture_current(
            &Database::default(),
            &view,
            definition,
            ConstantRequest::Constant,
            usize::MAX
        )
        .is_err()
    );
}
#[test]
fn same_written_bytes_in_external_module_keep_distinct_definition_identity() {
    let text = "const X:u32=22;";
    let unit = Digest::of_bytes(b"LRR current constant source acceptance");
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("module.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec!["child".into()],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec!["child".into()],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let definition = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "X")
        .unwrap()
        .def_key
        .fingerprint();
    let source = ConstantSource::capture_prepared(&view, definition).unwrap();
    let declaration = source
        .declarations()
        .iter()
        .find(|d| d.definition() == definition)
        .unwrap();
    assert_eq!(declaration.context().scope.module_path, vec!["child"]);
    let (_, root, root_definition) = world(text, "X", 2024, true);
    assert_ne!(definition, root_definition);
    assert_ne!(declaration.body_handle().def_key, root_definition);
    assert_eq!(root.selection().outline.module_path, Vec::<String>::new());
    // Raw prepared capture deliberately lacks current Type authority.
    assert!(source.constant(declaration.range()).is_err());
}
#[test]
fn unchanged_shadow_const_param_is_a_retained_prerequisite() {
    // Whole compile-fail diagnostics are not manufactured by this pure evaluator.
    let profile = ProfileIdentity::current();
    let blob = SourceBlob::new(SHADOW.as_bytes().to_vec()).unwrap();
    let source = ConstantSource::parse(
        &blob,
        profile,
        Digest::of_bytes(b"shadow source"),
        CfgSet::guest_default(),
    )
    .unwrap();
    assert!(!source.declarations().is_empty());
    for declaration in source.declarations() {
        assert!(source.const_call(declaration.range(), vec![]).is_err());
    }
}

#[test]
fn exact_body_context_and_cfg_publication_tampering_refuses() {
    for variant in 0..3 {
        let (mut db, view, definition) = world(NESTED, "X", 2024, true);
        let certificate = admit(&db, &view, definition).unwrap();
        let mut facts = view.prepared_inputs().clone();
        match variant {
            0 => {
                let InputValue::DefinitionBody(body) = facts
                    .get_mut(&InputKey::DefinitionBody(definition))
                    .unwrap()
                else {
                    panic!()
                };
                body.def_key = Digest::of_bytes(b"forged body owner");
            }
            1 => {
                let InputValue::DeclarationContext(context) = facts
                    .get_mut(&InputKey::DeclarationContext(definition))
                    .unwrap()
                else {
                    panic!()
                };
                context.scope.module_path.push("forged".into());
            }
            _ => {
                let InputValue::CrateCfg(cfg) = facts
                    .get_mut(&InputKey::CrateCfg(view.selection().outline.crate_key))
                    .unwrap()
                else {
                    panic!()
                };
                cfg.atoms.insert("forged_configuration".into());
            }
        }
        db.publish_inputs(facts).unwrap();
        assert!(admit(&db, &view, definition).is_err());
        assert!(certificate.revalidate(&db, &view).is_err());
    }
}

#[test]
fn cold_shadowed_type_declaration_does_not_force_header_semantics() {
    let text = "type i32=bool;const BAD:i32=true;const X:u32=22;fn main(){}";
    let (db, view, definition) = world(text, "X", 2024, true);
    let certificate = admit(&db, &view, definition).unwrap();
    assert_eq!(
        evaluate(certificate.plan(), ConstLimits::default())
            .unwrap()
            .value()
            .scalar()
            .unwrap()
            .payload,
        22
    );
    let (db, view, definition) = world(text, "BAD", 2024, true);
    assert!(admit(&db, &view, definition).is_err());
}

#[test]
fn copied_type_absence_certificate_wrong_owner_cannot_attach() {
    let (_, view, definition) = world(NESTED, "X", 2024, true);
    let source = ConstantSource::capture_prepared(&view, definition).unwrap();
    let mut selection = view.selection().clone();
    selection.outline.crate_key = Digest::of_bytes(b"wrong source owner");
    let certificate = CurrentConstantTypeCertificate {
        selection,
        scope: view.outline().root_scope.clone(),
        absent: ["u32".into()].into(),
        inert_attributes: BTreeMap::new(),
    };
    assert!(source.with_type_certificate(certificate).is_err());
}

#[test]
fn unchanged_array_length_request_binds_exact_function_body_all_editions() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/arithmetic-expr-in-array-len.rs"
    );
    let start = u32::try_from(text.find("2*4").unwrap()).unwrap();
    let range = SourceRange {
        start,
        end: start + 3,
    };
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(text, "main", edition, true);
        let certificate = ConstantSourceAdmission::capture_current(
            &db,
            &view,
            definition,
            ConstantRequest::ArrayLength(range),
            usize::MAX,
        )
        .unwrap();
        assert_eq!(
            evaluate(certificate.plan(), ConstLimits::default())
                .unwrap()
                .value()
                .scalar()
                .unwrap()
                .payload,
            8
        );
        let wrong = SourceRange {
            start,
            end: start + 2,
        };
        assert!(
            ConstantSourceAdmission::capture_current(
                &db,
                &view,
                definition,
                ConstantRequest::ArrayLength(wrong),
                usize::MAX
            )
            .is_err()
        );
    }
}

#[path = "constant_macro_admission_tests.rs"]
mod macro_tests;

#[path = "constant_source_multisource_tests.rs"]
mod multisource_tests;

#[path = "constant_query_tests.rs"]
mod query_tests;

#[path = "constant_path_selection_tests.rs"]
mod path_tests;

#[path = "constant_macro_demand_tests.rs"]
mod macro_demand_tests;
