//! Source-namespace component controls, not whole-fixture/guest execution.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::compile_input_catalog::CompileInputCatalog,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
};
use std::sync::Arc;
const APP: Digest = Digest([201; 32]);
const LIB: Digest = Digest([202; 32]);
const MAIN: &str = "renamed::seed!();fn main()->i64{made()}fn cold<T:Missing>(){unknown();}";
const LIBRARY: &str = "#[macro_export] macro_rules! seed{()=>{pub fn made()->i64{137_i64}}}";
fn prepared(
    main: &str,
    library: &str,
    edition: u16,
    publish_epoch: bool,
) -> (
    Database,
    FunctionSignature,
    ParseKey,
    Vec<PreparedSourceView>,
) {
    prepared_world(
        vec![
            ("app/main.rs", APP, main, edition),
            ("dependency/lib.rs", LIB, library, edition),
        ],
        vec![SourceDependency {
            consumer: APP,
            alias: "renamed".into(),
            provider: LIB,
        }],
        publish_epoch,
    )
}
pub(super) fn prepared_world(
    sources: Vec<(&str, Digest, &str, u16)>,
    rows: Vec<SourceDependency>,
    publish_epoch: bool,
) -> (
    Database,
    FunctionSignature,
    ParseKey,
    Vec<PreparedSourceView>,
) {
    let cfg = CfgSet::guest_default();
    let mut units = Vec::new();
    let mut selections = Vec::new();
    for (path, crate_key, text, edition) in sources {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let path = SourcePath::new(path).unwrap();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        selections.push(SourceSelection {
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
        });
        units.push(SourceUnitInput {
            path,
            crate_key,
            blob,
            profile,
            cfg: cfg.clone(),
            module_path: vec![],
        });
    }
    let inputs =
        PreparedSourceInputs::refresh_with_dependencies(None, units, Default::default(), rows)
            .unwrap();
    let sources = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(source) = inputs.select(selection).unwrap() else {
                panic!("actual source absent")
            };
            source
        })
        .collect::<Vec<_>>();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    if publish_epoch {
        db.begin_expansion_epoch(1).unwrap();
    }
    let mut discovery = SourceDiscovery::new(
        sources.iter().map(PreparedSourceView::share).collect(),
        sources[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([222; 32]),
        crate::expand::source_declarative::engine_version(),
        Arc::new(
            CompileInputCatalog::standalone(&sources[0].selection().outline.parse.profile).unwrap(),
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
            _ => panic!("actual source preparation incomplete"),
        }
    }
    discovery.publish(&mut db).unwrap();
    let stamps = db.lexical_namespace_stamps().unwrap();
    let unit = stamps
        .keys()
        .find_map(|key| match db.published_input(key) {
            Some((InputValue::GeneratedSyntax(unit), _))
                if unit
                    .definitions
                    .values()
                    .any(|definition| definition.declared_name == "made") =>
            {
                Some(unit.clone())
            }
            _ => None,
        })
        .unwrap();
    let definition = *unit
        .definitions
        .iter()
        .find(|(_, definition)| definition.declared_name == "made")
        .unwrap()
        .0;
    let InputValue::DefinitionHeader(header) = db
        .published_input(&InputKey::DefinitionHeader(definition))
        .unwrap()
        .0
    else {
        panic!()
    };
    let InputValue::DeclarationContext(context) = db
        .published_input(&InputKey::DeclarationContext(definition))
        .unwrap()
        .0
    else {
        panic!()
    };
    let InputValue::DefinitionBody(body) = db
        .published_input(&InputKey::DefinitionBody(definition))
        .unwrap()
        .0
    else {
        panic!()
    };
    let parsed = crate::syntax::parse::ParsedFile::from_generated_items(&unit.items).unwrap();
    let signature = parsed
        .function_signature(definition, header, context, Some(body))
        .unwrap();
    (db, signature, unit.key.clone(), sources)
}
#[test]
fn actual_renamed_dependency_root_template_has_both_current_type_namespaces() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, signature, parse, _sources) = prepared(MAIN, LIBRARY, edition, true);
        assert_eq!(signature.scope.crate_key, APP);
        let InputValue::GeneratedSyntax(unit) = db
            .published_input(&InputKey::GeneratedSyntax(parse.content))
            .unwrap()
            .0
        else {
            panic!()
        };
        assert!(
            unit.items
                .tokens()
                .spans()
                .iter()
                .any(|span| span.source.unit.0 == LIB.0)
        );
        let crate::expand::identity::MacroIdentity::Declaration { definition } =
            &unit.items.record().key().macro_identity
        else {
            panic!("actual declaration record required")
        };
        let source = _sources
            .iter()
            .find(|source| source.selection().outline.crate_key == LIB)
            .unwrap();
        let original = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == *definition)
            .unwrap();
        let scan = source
            .declaration_inventory(
                crate::syntax::declaration_scan::DeclarationScanLimits::default(),
            )
            .unwrap();
        let scanned = scan
            .macro_definitions()
            .iter()
            .find(|header| header.range == original.declaration_span)
            .unwrap();
        assert_ne!(
            scanned.identity, *definition,
            "scan and ordinary definition identities are different domains"
        );
        assert_eq!(
            original.def_key.item_kind,
            crate::source::manifest::ItemKind::Macro
        );
        assert_eq!(original.scope, source.outline().root_scope);
        require(&db, &signature, &parse).unwrap();
        // Structural result admission never asks for the generated or cold body.
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_)))
        );
    }
}
#[test]
fn either_actual_scope_shadow_is_required_blocker_not_primitive_permission() {
    for edition in [2015, 2018, 2021, 2024] {
        for (main, library) in [
            (format!("type i64=bool;{MAIN}"), LIBRARY.to_owned()),
            (MAIN.to_owned(), format!("type i64=bool;{LIBRARY}")),
        ] {
            let (db, signature, parse, _sources) = prepared(&main, &library, edition, true);
            let error = require(&db, &signature, &parse).unwrap_err();
            assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
            assert_eq!(
                error.message,
                "primitive result requires its exact current generated Type namespace"
            );
        }
    }
}
#[test]
fn genuine_foreign_origin_still_requires_actual_published_epoch() {
    let (db, signature, parse, _sources) = prepared(MAIN, LIBRARY, 2024, false);
    let error = require(&db, &signature, &parse).unwrap_err();
    assert_eq!(
        error.message,
        "Type receipt requires its published matching ExpansionEpoch"
    );
}
#[test]
fn genuine_source_universe_replacement_removes_old_generated_authority() {
    let (mut db, signature, parse, _sources) = prepared(MAIN, LIBRARY, 2024, true);
    require(&db, &signature, &parse).unwrap();
    let extra = SourceUnitInput {
        path: SourcePath::new("other/lib.rs").unwrap(),
        crate_key: Digest([203; 32]),
        blob: SourceBlob::new(b"type i64=bool;".to_vec()).unwrap(),
        profile: ProfileIdentity::current(),
        cfg: CfgSet::guest_default(),
        module_path: vec![],
    };
    PreparedSourceInputs::refresh_with_dependencies(None, vec![extra], Default::default(), vec![])
        .unwrap()
        .publish(&mut db)
        .unwrap();
    assert!(
        db.published_input(&InputKey::GeneratedSyntax(parse.content))
            .is_none()
    );
    assert!(
        db.published_input(&InputKey::DefinitionSource(signature.definition))
            .is_none()
    );
    let error = require(&db, &signature, &parse).unwrap_err();
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert_eq!(
        error.message,
        "primitive result requires its exact current generated Type namespace"
    );
}

#[test]
fn expired_original_source_lease_does_not_become_namespace_authority() {
    let (db, signature, parse, sources) = prepared(MAIN, LIBRARY, 2024, true);
    require(&db, &signature, &parse).unwrap();
    drop(sources);
    let error = require(&db, &signature, &parse).unwrap_err();
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert_eq!(
        error.message,
        "primitive result requires its exact current generated Type namespace"
    );
}
