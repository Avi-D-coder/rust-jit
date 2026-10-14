//! Real source/expansion/header components only; no evaluator, layout or main execution.
use super::*;
use crate::frontend::{
    generated_constant_provenance::CurrentGeneratedConstant,
    generated_type_admission::{
        GeneratedTypeOutcome, GeneratedTypePrerequisite, SelectedGeneratedType,
    },
};
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

#[test]
fn unchanged_scalar_default_prelude_uses_published_current_type_prelude_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, parent) = world(VIS, edition, &["with_pub"]);
        assert!(matches!(
            SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap(),
            GeneratedTypeOutcome::Need(GeneratedTypePrerequisite::PrimitiveEnvironment)
        ));
        let GeneratedPrimitiveOutcome::Ready(proof) =
            CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                .unwrap()
        else {
            panic!("genuine default Type prelude now supplies exact absence")
        };
        assert_eq!(proof.primitive(), PrimitiveType::I32);
    }
}
#[test]
fn actual_disabled_prelude_keeps_language_primitive_route_in_every_edition() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "#![no_implicit_prelude]macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
            "#![no_std]#![no_implicit_prelude]macro_rules!m{()=>{const A:bool=true;}}m!();fn main(){}",
        ] {
            let (db, owner, parent) = world(text, edition, &[]);
            let GeneratedPrimitiveOutcome::Ready(proof) =
                CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                    .unwrap()
            else {
                panic!("actual primitive environment must be established")
            };
            let expected = if text.contains(":bool") {
                PrimitiveType::Bool
            } else {
                PrimitiveType::I32
            };
            assert_eq!(proof.primitive(), expected);
            assert_eq!(proof.profile().edition, edition);
            assert_ne!(proof.member_identity(), Digest([0; 32]));
            proof
                .revalidate(&db, &owner, &parent, &[], MAX_BYTES)
                .unwrap();
        }
    }
}
#[test]
fn actual_alias_and_nominal_shadow_never_use_language_primitive_facet() {
    for text in [
        "#![no_implicit_prelude]type i32=bool;macro_rules!m{()=>{const A:i32=true;}}m!();fn main(){}",
        "#![no_implicit_prelude]struct i32;macro_rules!m{()=>{const A:i32=i32;}}m!();fn main(){}",
    ] {
        let (db, owner, parent) = world(text, 2024, &[]);
        let GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::SourceType(
            NameLookup::Defined(def),
        )) = CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
            .unwrap()
        else {
            panic!("real shadow identity must survive")
        };
        assert_ne!(def.identity, parent.retained().member().fingerprint());
        assert_eq!(
            def.defining_unit,
            owner.module_source().selection().outline.crate_key
        );
    }
}
#[test]
fn missing_and_foreign_profile_catalogue_cannot_grant_a_language_facet() {
    let (db, mut owner, parent) = world(
        "#![no_implicit_prelude]macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
        2024,
        &[],
    );
    owner.compiler = None;
    assert!(matches!(
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).unwrap(),
        GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::CompilerPrimitiveFacet)
    ));
    let wrong = ProfileIdentity {
        edition: 2015,
        ..ProfileIdentity::current()
    };
    owner.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&wrong).unwrap(),
    ));
    assert!(
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).is_err()
    );
}
#[test]
fn unsupported_qualified_and_unknown_types_keep_precise_source_prerequisites() {
    for (text, qualified) in [
        (
            "#![no_implicit_prelude]macro_rules!m{()=>{const A:core::primitive::i32=1;}}m!();fn main(){}",
            true,
        ),
        (
            "#![no_implicit_prelude]macro_rules!m{()=>{const A:NotAType=1;}}m!();fn main(){}",
            false,
        ),
    ] {
        let (db, owner, parent) = world(text, 2024, &[]);
        let actual =
            CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                .unwrap();
        if qualified {
            assert!(matches!(
                actual,
                GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::WrittenType)
            ));
        } else {
            assert!(matches!(
                actual,
                GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::UnknownPrimitive)
            ));
        }
    }
}
#[test]
fn current_epoch_owner_quota_and_private_field_tamper_refuse_replay() {
    let text = "#![no_implicit_prelude]macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}";
    let (mut db, owner, parent) = world(text, 2024, &[]);
    assert!(CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], 0).is_err());
    let GeneratedPrimitiveOutcome::Ready(mut proof) =
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).unwrap()
    else {
        panic!()
    };
    let (_, foreign, foreign_parent) = world_in_unit(text, 2024, &[], Digest([216; 32]));
    assert!(
        proof
            .revalidate(&db, &foreign, &foreign_parent, &[], MAX_BYTES)
            .is_err()
    );
    let original = proof.member;
    proof.member = Digest([213; 32]);
    assert!(
        proof
            .revalidate(&db, &owner, &parent, &[], MAX_BYTES)
            .is_err()
    );
    proof.member = original;
    proof.context = Key([214; 32]);
    assert!(
        proof
            .revalidate(&db, &owner, &parent, &[], MAX_BYTES)
            .is_err()
    );
    db.begin_expansion_epoch(2).unwrap();
    assert!(
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).is_err()
    );
}
#[test]
fn genuine_opaque_origin_requires_its_definition_type_seal_before_any_call_site_shadow() {
    let text = "#![feature(decl_macro)]#![no_implicit_prelude]pub macro make(){const A:i32=1;} mod call{type i32=bool;crate::make!();}fn main(){}";
    let (mut db, owner, parent) = world(text, 2024, &["call"]);
    let GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::TypeNamespace(scope)) =
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).unwrap()
    else {
        panic!("definition Type scope must not be replaced by caller scope")
    };
    assert!(scope.module_path.is_empty());
    let mut discovery = SourceDiscovery::new(
        vec![owner.module_source().share()],
        scope,
        crate::source::manifest::Namespace::Type,
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
            _ => panic!("genuine root Type preparation required"),
        }
    }
    discovery.publish(&mut db).unwrap();
    // Genuine between-run publication requires reminting the preserved parent.
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
    assert!(proof.scope().module_path.is_empty());
    assert_ne!(
        proof.context,
        Key(parent.retained().record().key().parent_context.0)
    );
}
#[test]
fn pinned_builtin_types_exist_independently_of_stdlib_prelude_policy() {
    let hir =
        include_str!("../../../../fixtures/upstream/rustc-checkout/compiler/rustc_hir/src/hir.rs");
    let ident = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/compiler/rustc_resolve/src/ident.rs"
    );
    assert!(hir.contains("pub const ALL: [Self; 19]"));
    assert!(ident.contains("Scope::BuiltinTypes => true"));
    let unavailable = crate::frontend::compile_input_catalog::CompileInputCatalog::new(
        Digest([215; 32]),
        2024,
        [],
    )
    .unwrap();
    assert!(
        unavailable
            .primitive_facet(&ProfileIdentity::current())
            .unwrap()
            .is_none()
    );
}

#[test]
fn unchanged_original_policy_and_nominal_shadow_are_distinct_source_facts() {
    for (text, edition, path) in [
        (
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/no_implicit_prelude.rs"
            ),
            2015,
            "no-implicit-2015.rs",
        ),
        (
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/no_implicit_prelude-2018.rs"
            ),
            2018,
            "no-implicit-2018.rs",
        ),
        (
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/no_implicit_prelude-2021.rs"
            ),
            2021,
            "no-implicit-2021.rs",
        ),
    ] {
        let (_, views, _) = prepare(vec![input(text, path, Digest([217; 32]), edition)], vec![]);
        let actual = PreparedCratePreludePolicy::capture_current(&views[0], 1, MAX_BYTES).unwrap();
        let policy = actual.policy();
        assert_eq!(
            policy.disabled,
            if edition == 2021 {
                vec![Vec::<String>::new()]
            } else {
                vec![vec!["bar".to_owned()]]
            }
        );
        assert_eq!(actual.owning_root_selection(), views[0].selection());
        // Actual policy components do not certify a Type scope or original fixture.
    }
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/shadowed/primitive-type-shadowing.rs"
    );
    let (_, views, _) = prepare(
        vec![input(text, "primitive-shadow.rs", Digest([218; 32]), 2024)],
        vec![],
    );
    let mut names =
        crate::frontend::names::NameNamespaces::from_outlines(&[views[0].outline()]).unwrap();
    names.resolve_imports().unwrap();
    let actual = names
        .lookup_unqualified(
            Namespace::Type,
            "bool",
            &views[0].outline().root_scope,
            2024,
            None,
        )
        .unwrap();
    let NameLookup::Defined(def) = actual else {
        panic!("original nominal bool declaration must survive")
    };
    let nominal = views[0]
        .outline()
        .items
        .iter()
        .find(|i| {
            i.def_key.declared_name == "bool"
                && i.def_key.item_kind == crate::source::manifest::ItemKind::Struct
        })
        .unwrap();
    assert_eq!(def.identity, nominal.def_key.fingerprint());
    assert_ne!(
        def.identity,
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &views[0].outline().profile
        )
        .unwrap()
        .primitive_facet(&views[0].outline().profile)
        .unwrap()
        .unwrap()
        .member_identity(PrimitiveType::Bool)
    );
}

#[test]
fn actual_published_type_receipt_absence_and_foreign_copy_cannot_mint_primitive() {
    let text = "#![no_implicit_prelude]macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}";
    for foreign in [false, true] {
        let (mut db, owner, parent) = world(text, 2024, &[]);
        let key = InputKey::DiscoverySeal(parent.seal_keys().1.clone());
        let Some((InputValue::DiscoverySeal(current), _)) = db.published_input(&key) else {
            panic!()
        };
        assert!(
            current
                .compiler_primitive_receipt(owner.module_source().selection())
                .unwrap()
                .is_some()
        );
        let mut replacement = current.clone();
        replacement.compiler_primitive_receipts.clear();
        if foreign {
            let (other_db, other, other_parent) = world_in_unit(text, 2024, &[], Digest([219; 32]));
            let Some((InputValue::DiscoverySeal(other_input), _)) = other_db
                .published_input(&InputKey::DiscoverySeal(other_parent.seal_keys().1.clone()))
            else {
                panic!()
            };
            let receipt = other_input
                .compiler_primitive_receipt(other.module_source().selection())
                .unwrap()
                .unwrap();
            assert_ne!(receipt.source(), owner.module_source().selection());
            replacement
                .compiler_primitive_receipts
                .push(receipt.clone());
        }
        // Replay the actual sealed Type publication with its genuine retained
        // records. Raw publish_inputs is forbidden for derived DiscoverySeal.
        let records = replacement
            .records
            .iter()
            .map(|descriptor| {
                let Some((InputValue::ExpansionRecord(record), _)) =
                    db.published_input(&InputKey::ExpansionRecord(descriptor.key.fingerprint()))
                else {
                    panic!()
                };
                assert_eq!(record.descriptor().fingerprint(), descriptor.fingerprint());
                Arc::new(record.clone())
            })
            .collect::<Vec<_>>();
        let publication = db.publish_discovery(*replacement, &records, &[], &[], BTreeMap::new());
        if foreign {
            assert_eq!(publication.unwrap_err().code, "LRR-E-GENERATED-PRIMITIVE");
            parent.revalidate(&db, &owner).unwrap();
            continue;
        }
        publication.unwrap();
        assert!(parent.revalidate(&db, &owner).is_err());
        let parent = CurrentGeneratedConstant::capture(
            &db,
            &owner,
            parent.retained().member().fingerprint(),
            MAX_BYTES,
        )
        .unwrap();
        let trace = db.trace.events.len();
        assert!(matches!(
            CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES)
                .unwrap(),
            GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::CompilerPrimitiveFacet)
        ));
        assert_eq!(trace, db.trace.events.len());
    }
}
#[test]
fn real_cfg_publication_barrier_refuses_before_any_type_or_query_authority() {
    let (mut db, owner, parent) = world(
        "#![no_implicit_prelude]macro_rules!m{()=>{const A:i32=1;}}m!();fn main(){}",
        2024,
        &[],
    );
    let GeneratedPrimitiveOutcome::Ready(proof) =
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).unwrap()
    else {
        panic!()
    };
    let mut cfg = owner.module_source().captured_macro_cfg().unwrap().clone();
    cfg.atoms.insert("changed_owning_cfg".into());
    db.publish_inputs(BTreeMap::from([(
        InputKey::CrateCfg(owner.module_source().selection().outline.crate_key),
        InputValue::CrateCfg(cfg),
    )]))
    .unwrap();
    let trace = db.trace.events.len();
    assert!(
        proof
            .revalidate(&db, &owner, &parent, &[], MAX_BYTES)
            .is_err()
    );
    assert!(
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).is_err()
    );
    assert_eq!(trace, db.trace.events.len());
}

#[test]
fn actual_generated_inline_module_keeps_unestablished_owning_policy_explicit() {
    let (db, owner, parent) = world(
        "#![no_implicit_prelude]macro_rules!m{()=>{mod made{const A:i32=1;}}}m!();fn main(){}",
        2024,
        &["made"],
    );
    assert!(matches!(
        CurrentGeneratedPrimitive::capture_current(&db, &owner, &parent, &[], MAX_BYTES).unwrap(),
        GeneratedPrimitiveOutcome::Need(GeneratedPrimitivePrerequisite::GeneratedScopePolicy)
    ));
}
