//! Real source/expansion/header components only; no evaluator, layout or main execution.
use super::*;
use crate::frontend::generated_constant_provenance::CurrentGeneratedConstant;
use crate::frontend::generated_primitive_admission::{
    CurrentGeneratedPrimitive, GeneratedPrimitiveOutcome, GeneratedPrimitivePrerequisite,
};
use crate::ir::op::PrimitiveType;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const VIS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-pub-matcher.rs"
);
fn input(text: &str, path: &str, unit: Digest, edition: u16) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: unit,
        module_path: vec![],
        profile: ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    }
}
fn prepare(
    units: Vec<SourceUnitInput>,
    edges: Vec<SourceDependency>,
) -> (Database, Vec<PreparedSourceView>, Vec<SourceDeclarative>) {
    let selections = units
        .iter()
        .map(|u| SourceSelection {
            path: u.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: u.blob.content(),
                    profile: u.profile.clone(),
                },
                crate_key: u.crate_key,
                module_path: u.module_path.clone(),
                cfg: u.cfg.fingerprint(),
            },
        })
        .collect::<Vec<_>>();
    let inputs =
        PreparedSourceInputs::refresh_with_dependencies(None, units, Default::default(), edges)
            .unwrap();
    let views = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(v) = inputs.select(s).unwrap() else {
                panic!()
            };
            v
        })
        .collect::<Vec<_>>();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let providers = views
        .iter()
        .map(|v| SourceDeclarative::new(v.share()).unwrap())
        .collect();
    (db, views, providers)
}
fn world(
    text: &str,
    edition: u16,
    module: &[&str],
) -> (Database, SourceDeclarative, CurrentGeneratedConstant) {
    world_in_unit(text, edition, module, Digest([172; 32]))
}
fn world_in_unit(
    text: &str,
    edition: u16,
    module: &[&str],
    unit: Digest,
) -> (Database, SourceDeclarative, CurrentGeneratedConstant) {
    let (mut db, views, providers) =
        prepare(vec![input(text, "type-current.rs", unit, edition)], vec![]);
    let mut scope = views[0].outline().root_scope.clone();
    scope.module_path.extend(module.iter().map(|m| (*m).into()));
    let mut definition = None;
    for namespace in [
        crate::source::manifest::Namespace::Value,
        crate::source::manifest::Namespace::Type,
    ] {
        let mut discovery = SourceDiscovery::new(
            vec![views[0].share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"type current world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"type current compile env"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &views[0].selection().outline.parse.profile,
                )
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
                    "actual type preparation frontier: {} {}",
                    diagnostic.code, diagnostic.message
                ),
                _ => panic!("type source preparation is unestablished"),
            }
        }
        if namespace == crate::source::manifest::Namespace::Value {
            let NameLookup::Defined(found) = discovery.lookup("A").unwrap() else {
                panic!()
            };
            definition = Some(found.identity);
        }
        discovery.publish(&mut db).unwrap();
    }
    let mut owner = providers.into_iter().next().unwrap();
    owner.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(owner.profile())
            .unwrap(),
    ));
    let parent =
        CurrentGeneratedConstant::capture(&db, &owner, definition.unwrap(), MAX_BYTES).unwrap();
    (db, owner, parent)
}

fn target(db: &mut Database, owner: &SourceDeclarative, module: &[&str]) {
    let mut scope = owner.module_source().outline().root_scope.clone();
    scope
        .module_path
        .extend(module.iter().map(|m| m.to_string()));
    let mut discovery = SourceDiscovery::new(
        vec![owner.module_source().share()],
        scope,
        Namespace::Type,
        Digest::of_bytes(b"type current world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"type current compile env"),
        crate::expand::source_declarative::engine_version(),
        owner.compiler.clone().unwrap(),
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
            _ => panic!("actual target Type preparation required"),
        }
    }
    discovery.publish(db).unwrap();
}
#[test]
fn unchanged_default_generated_i32_joins_compiler_type_prelude_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, parent) = world(VIS, edition, &["with_pub"]);
        let GeneratedPrimitiveOutcome::Ready(proof) =
            CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                .unwrap()
        else {
            panic!("actual compiler default prelude must prove i32 absence")
        };
        assert_eq!(proof.primitive(), PrimitiveType::I32);
        assert_eq!(proof.profile().edition, edition);
        proof
            .revalidate(&db, &owner, &parent, &[], MAX_BYTES)
            .unwrap();
    }
}
#[test]
fn no_std_and_disabled_policy_use_their_actual_owned_profile() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "#![no_std]macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
            "#![no_implicit_prelude]macro_rules!m{()=>{const A:char='a';}}m!();fn main(){}",
        ] {
            let (db, owner, parent) = world(text, edition, &[]);
            let GeneratedPrimitiveOutcome::Ready(proof) =
                CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                    .unwrap()
            else {
                panic!()
            };
            assert_eq!(proof.profile().edition, edition);
        }
    }
}
#[test]
fn genuine_custom_module_requires_target_seal_then_exact_absence() {
    let text = "#![feature(prelude_import)]mod p{pub struct Present;}#[prelude_import]use crate::p::*;macro_rules!m{()=>{const A:i32=1;}}mod c{m!();}fn main(){}";
    let (mut db, owner, parent) = world(text, 2024, &["c"]);
    assert!(
        matches!(CurrentGeneratedPrimitive::capture_current(&db,&owner,&parent,&[],MAX_BYTES).unwrap(),GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::TypePreludeSelection(TypePreludePrerequisite::TargetTypeNamespace(scope)))if scope.module_path==vec!["p"])
    );
    target(&mut db, &owner, &["p"]);
    assert!(parent.revalidate(&db, &owner).is_err());
    let parent = CurrentGeneratedConstant::capture(
        &db,
        &owner,
        parent.retained().member().fingerprint(),
        MAX_BYTES,
    )
    .unwrap();
    let GeneratedPrimitiveOutcome::Ready(proof) =
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).unwrap()
    else {
        panic!()
    };
    assert_eq!(proof.primitive(), PrimitiveType::I32);
    proof
        .revalidate(&db, &owner, &parent, &[], MAX_BYTES)
        .unwrap();
}
#[test]
fn custom_source_member_identity_never_becomes_primitive_from_equal_spelling() {
    let text = "#![feature(prelude_import)]mod p{pub type i32=bool;}#[prelude_import]use crate::p::*;macro_rules!m{()=>{const A:i32=true;}}mod c{m!();}fn main(){}";
    let (mut db, owner, parent) = world(text, 2024, &["c"]);
    target(&mut db, &owner, &["p"]);
    let parent = CurrentGeneratedConstant::capture(
        &db,
        &owner,
        parent.retained().member().fingerprint(),
        MAX_BYTES,
    )
    .unwrap();
    assert!(
        matches!(CurrentGeneratedPrimitive::capture_current(&db,&owner,&parent,&[],MAX_BYTES).unwrap(),GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::TypePreludeSelection(TypePreludePrerequisite::Member(NameLookup::Defined(found))))if found.defining_unit==owner.module_source().selection().outline.crate_key)
    );
}
#[test]
fn actual_custom_import_alias_keeps_selected_module_identity() {
    let text = "#![feature(prelude_import)]mod p{pub struct Present;}use crate::p as chosen;#[prelude_import]use chosen::*;macro_rules!m{()=>{const A:i32=1;}}mod c{m!();}fn main(){}";
    let (db, owner, parent) = world(text, 2024, &["c"]);
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(parent.seal_keys().1.clone()))
    else {
        panic!()
    };
    let receipt = input
        .type_prelude_receipt(owner.module_source().selection(), parent.retained().scope())
        .unwrap()
        .unwrap();
    let PreparedMode::Custom {
        module, directive, ..
    } = &receipt.mode
    else {
        panic!("actual alias route must select original module")
    };
    assert_eq!(module.module_path, vec!["p"]);
    assert!(
        directive
            .import()
            .path
            .iter()
            .any(|p| matches!(p,crate::syntax::imports::ImportSegment::Name(n)if n=="chosen"))
    );
}
#[test]
fn multiple_custom_imports_and_generated_directives_remain_explicit() {
    for (text, generated) in [
        (
            "#![feature(prelude_import)]mod p{}mod q{}#[prelude_import]use crate::p::*;#[prelude_import]use crate::q::*;macro_rules!m{()=>{const A:i32=1;}}mod c{m!();}fn main(){}",
            false,
        ),
        (
            "#![feature(prelude_import)]mod p{}macro_rules!m{()=>{#[prelude_import]use crate::p::*;const A:i32=1;}}m!();fn main(){}",
            true,
        ),
    ] {
        let (db, owner, parent) = world(text, 2024, if generated { &[] } else { &["c"] });
        let actual =
            CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                .unwrap();
        if generated {
            assert!(matches!(
                actual,
                GeneratedPrimitiveOutcome::Need(
                    GeneratedPrimitivePrerequisite::TypePreludeSelection(
                        TypePreludePrerequisite::GeneratedDirective(_)
                    )
                )
            ))
        } else {
            assert!(matches!(
                actual,
                GeneratedPrimitiveOutcome::Need(
                    GeneratedPrimitivePrerequisite::TypePreludeSelection(
                        TypePreludePrerequisite::MultipleDirectives
                    )
                )
            ))
        }
    }
}
#[test]
fn compiler_catalogue_membership_and_prelude_spellings_are_edition_specific() {
    for edition in [2015, 2018, 2021, 2024] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let compiler =
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                .unwrap();
        let facet = compiler.type_prelude_facet(&profile).unwrap().unwrap();
        assert!(!facet.contains_export_spelling(true, "i32"));
        assert!(!facet.contains_export_spelling(false, "bool"));
        assert!(facet.contains_export_spelling(true, "Vec"));
        assert!(!facet.contains_export_spelling(false, "Vec"));
        assert_eq!(
            facet.contains_export_spelling(false, "TryInto"),
            edition >= 2021
        );
        assert_eq!(
            facet.contains_export_spelling(true, "Future"),
            edition == 2024
        );
    }
}
#[test]
fn exact_epoch_profile_and_budget_refusal_have_no_provider_effect() {
    let (mut db, owner, parent) = world(
        "macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
        2024,
        &[],
    );
    let trace = db.trace.events.len();
    assert!(CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], 0).is_err());
    let TypePreludeAbsenceOutcome::Absent(proof) = CurrentTypePreludeAbsence::capture_current(
        &db,
        &owner,
        parent.retained().scope(),
        parent.seal_keys().1,
        "i32",
        MAX_BYTES,
    )
    .unwrap() else {
        panic!()
    };
    assert!(
        proof
            .revalidate(
                &db,
                &owner,
                parent.retained().scope(),
                parent.seal_keys().1,
                "char",
                MAX_BYTES
            )
            .is_err()
    );
    db.begin_expansion_epoch(2).unwrap();
    assert!(
        proof
            .revalidate(
                &db,
                &owner,
                parent.retained().scope(),
                parent.seal_keys().1,
                "i32",
                MAX_BYTES
            )
            .is_err()
    );
    assert_eq!(trace, db.trace.events.len());
}
#[test]
fn pinned_custom_and_legacy_hygiene_originals_retain_real_directives() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/stdlib-prelude-from-opaque-early.rs"
    );
    let (_, views, _) = prepare(
        vec![input(text, "custom-original.rs", Digest([224; 32]), 2024)],
        vec![],
    );
    let parsed = ParsedFile::parse(
        views[0].captured_macro_blob().unwrap(),
        views[0].outline().profile.clone(),
    )
    .unwrap();
    let TypePreludeSites::Captured(directives) = parsed
        .type_prelude_sites(
            views[0].outline(),
            views[0].captured_macro_cfg().unwrap(),
            MAX_BYTES,
        )
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(directives.len(), 1);
    assert!(matches!(
        directives[0].import().kind,
        crate::syntax::imports::ImportKind::Glob
    ));
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/prelude-import-hygiene.rs"
    );
    let (_, views, _) = prepare(
        vec![input(
            text,
            "injection-original.rs",
            Digest([225; 32]),
            2015,
        )],
        vec![],
    );
    let parsed = ParsedFile::parse(
        views[0].captured_macro_blob().unwrap(),
        views[0].outline().profile.clone(),
    )
    .unwrap();
    let TypePreludeSites::Captured(directives) = parsed
        .type_prelude_sites(
            views[0].outline(),
            views[0].captured_macro_cfg().unwrap(),
            MAX_BYTES,
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(directives.is_empty());
    // Imports named prelude_import are not an actual compiler-injected marker.
}

#[test]
fn genuine_compiler_attribute_shadow_cannot_become_a_custom_prelude_marker() {
    let text = "#![feature(prelude_import)]extern crate core;use core::prelude::v1::test as prelude_import;mod p{}#[prelude_import]use crate::p::*;fn main(){}";
    let (_, views, _) = prepare(
        vec![input(text, "attribute-shadow.rs", Digest([226; 32]), 2024)],
        vec![],
    );
    let mut discovery = SourceDiscovery::new(
        vec![views[0].share()],
        views[0].outline().root_scope.clone(),
        Namespace::Type,
        Digest::of_bytes(b"type current world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"type current compile env"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &views[0].outline().profile,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        discovery
            .poll()
            .err()
            .expect("exact Test vs builtin inert conflict")
            .code,
        "LRR-E-MACRO-AMBIGUOUS"
    );
}
#[test]
fn actual_module_import_ambiguity_is_preserved_as_a_typed_prelude_prerequisite() {
    let text = "#![feature(prelude_import)]mod x{pub mod p{}}mod y{pub mod p{}}use crate::x::p as chosen;use crate::y::p as chosen;#[prelude_import]use chosen::*;macro_rules!m{()=>{const A:i32=1;}}mod c{m!();}fn main(){}";
    let (db, owner, parent) = world(text, 2024, &["c"]);
    assert!(
        matches!(CurrentGeneratedPrimitive::capture_current(&db,&owner,&parent,&[],MAX_BYTES).unwrap(),GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::TypePreludeSelection(TypePreludePrerequisite::Module(ModuleLookup::Ambiguous(paths))))if paths.len()==2)
    );
}
#[test]
fn published_receipt_replay_is_exactly_bound_to_source_profile_and_written_name() {
    let (db, owner, parent) = world(
        "macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
        2024,
        &[],
    );
    let TypePreludeAbsenceOutcome::Absent(mut proof) = CurrentTypePreludeAbsence::capture_current(
        &db,
        &owner,
        parent.retained().scope(),
        parent.seal_keys().1,
        "i32",
        MAX_BYTES,
    )
    .unwrap() else {
        panic!()
    };
    proof.receipt = Digest([227; 32]);
    assert!(
        proof
            .revalidate(
                &db,
                &owner,
                parent.retained().scope(),
                parent.seal_keys().1,
                "i32",
                MAX_BYTES
            )
            .is_err()
    );
    let (_, foreign, foreign_parent) = world_in_unit(
        "macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
        2015,
        &[],
        Digest([228; 32]),
    );
    assert!(
        CurrentTypePreludeAbsence::capture_current(
            &db,
            &foreign,
            foreign_parent.retained().scope(),
            parent.seal_keys().1,
            "i32",
            MAX_BYTES
        )
        .is_err()
    );
}
