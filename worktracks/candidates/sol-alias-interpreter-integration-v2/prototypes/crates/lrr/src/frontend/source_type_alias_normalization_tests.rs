//! Source/header/selection components, not whole-main or compiler-tool fixture passes.
use super::*;
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
const FONT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/traits/trait-implementation-for-primitive-type-5280.rs"
);
const SHADOW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/shadowed/primitive-type-shadowing.rs"
);
const CYCLE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/checked-type-alias/variance-overflow.rs"
);
const GENERIC: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/type/type-alias-bounds.rs");
const CONST_PARAM: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/const-generics/early/const-param-shadowing.rs"
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

fn current(text: &str, edition: u16) -> (Database, SourceDeclarative, ScopeKey, SealKey) {
    let (mut db, views, mut owners) = prepare(
        vec![input(text, "alias.rs", Digest([192; 32]), edition)],
        vec![],
    );
    let mut owner = owners.remove(0);
    owner.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(owner.profile())
            .unwrap(),
    ));
    let scope = views[0].outline().root_scope.clone();
    for namespace in [Namespace::Value, Namespace::Type] {
        let mut discovery = SourceDiscovery::new(
            vec![views[0].share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"alias current world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"alias compile environment"),
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
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                    "actual source preparation prerequisite: {} {}",
                    diagnostic.code, diagnostic.message
                ),
                _ => panic!("actual source preparation unestablished"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    let key = SealKey {
        domain: SealDomain::Scope {
            id: scope.fingerprint(),
            namespace: Namespace::Type,
        },
        unit_graph: Digest::of_bytes(b"alias current world"),
        epoch: 1,
    };
    (db, owner, scope, key)
}
fn current_module(
    text: &str,
    edition: u16,
    module: &str,
) -> (Database, SourceDeclarative, ScopeKey, SealKey) {
    let (mut db, views, mut owners) = prepare(
        vec![input(
            text,
            "module-alias-original.rs",
            Digest([180; 32]),
            edition,
        )],
        vec![],
    );
    let mut owner = owners.remove(0);
    owner.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(owner.profile())
            .unwrap(),
    ));
    let scope = views[0]
        .outline()
        .scopes
        .keys()
        .find(|s| s.owner.is_none() && s.module_path == [module])
        .unwrap()
        .clone();
    let mut discovery = SourceDiscovery::new(
        vec![views[0].share()],
        scope.clone(),
        Namespace::Type,
        Digest::of_bytes(b"original module alias world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"original module alias compile environment"),
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
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                "original module preparation prerequisite: {} {}",
                diagnostic.code, diagnostic.message
            ),
            _ => panic!("original module requires unadmitted effectful producer"),
        }
    }
    discovery.publish(&mut db).unwrap();
    let key = SealKey {
        domain: SealDomain::Scope {
            id: scope.fingerprint(),
            namespace: Namespace::Type,
        },
        unit_graph: Digest::of_bytes(b"original module alias world"),
        epoch: 1,
    };
    (db, owner, scope, key)
}

fn resolve(
    db: &Database,
    owner: &SourceDeclarative,
    scope: &ScopeKey,
    key: &SealKey,
    name: &str,
) -> SourceAliasOutcome {
    CurrentSourcePrimitiveAlias::capture_original_current(
        db,
        owner,
        scope,
        key,
        name,
        &[],
        MAX_BYTES,
    )
    .unwrap()
}
#[test]
fn unchanged_font_alias_keeps_exact_source_member_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, scope, key) = current(FONT, edition);
        let SourceAliasOutcome::Ready(proof) = resolve(&db, &owner, &scope, &key, "FontTableTag")
        else {
            panic!("actual original primitive alias component")
        };
        assert_eq!(proof.primitive(), PrimitiveType::U32);
        let defs = proof.alias_definitions().collect::<Vec<_>>();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].declared_name, "FontTableTag");
        assert_eq!(defs[0].crate_key, scope.crate_key);
        proof.revalidate_inputs(&db).unwrap();
        assert!(!proof.dependencies().is_empty());
    }
}
#[test]
fn unchanged_primitive_shadow_is_a_source_nominal_frontier() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, scope, key) = current(SHADOW, edition);
        let SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(NameLookup::Defined(found))) =
            resolve(&db, &owner, &scope, &key, "bool")
        else {
            panic!("nominal shadow cannot become Bool")
        };
        let original = owner
            .module_source()
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "bool")
            .unwrap();
        assert_eq!(found.identity, original.def_key.fingerprint());
        assert_eq!(original.def_key.item_kind, ItemKind::Struct);
    }
}
#[test]
fn unchanged_direct_self_alias_cycle_is_reached_and_bounded() {
    let (db, owner, scope, key) = current(CYCLE, 2024);
    let error = CurrentSourcePrimitiveAlias::capture_original_current(
        &db,
        &owner,
        &scope,
        &key,
        "Second",
        &[],
        MAX_BYTES,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-TYPE-ALIAS-CYCLE");
}
#[test]
fn unchanged_const_parameter_shadow_does_not_change_global_alias_type() {
    let (db, owner, scope, key) = current(CONST_PARAM, 2024);
    let SourceAliasOutcome::Ready(proof) = resolve(&db, &owner, &scope, &key, "N") else {
        panic!("actual global alias header component")
    };
    assert_eq!(proof.primitive(), PrimitiveType::U32);
    assert_eq!(proof.alias_definitions().next().unwrap().declared_name, "N");
}
#[test]
fn unchanged_generic_alias_header_preserves_substitution_frontier_without_a_seal() {
    let (db, views, _) = prepare(
        vec![input(
            GENERIC,
            "generic-original.rs",
            Digest([190; 32]),
            2024,
        )],
        vec![],
    );
    let source = &views[0];
    let item = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "SVec")
        .unwrap();
    let parsed = db
        .prepared_parse(&source.selection().outline.parse)
        .unwrap();
    let recipe = parsed
        .type_alias_recipe(
            item.def_key.fingerprint(),
            &item.header,
            &DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
        )
        .unwrap();
    assert_eq!(recipe.generics[0].name, "T");
    assert!(!recipe.generics[0].bounds.is_empty());
    assert!(!recipe.target.is_empty());
    // This is retained-header evidence only: std::rc selection/full source preparation remains required.
}
#[test]
fn ordered_alias_chain_and_symbolic_primitive_spelling_keep_namespace_authority() {
    let text = "type i64=bool;type Mid=i64;type Final=Mid;fn cold(){missing();}fn main(){}";
    let (db, owner, scope, key) = current(text, 2024);
    let SourceAliasOutcome::Ready(proof) = resolve(&db, &owner, &scope, &key, "Final") else {
        panic!("actual aliases must normalize")
    };
    assert_eq!(proof.primitive(), PrimitiveType::Bool);
    assert_eq!(
        proof
            .alias_definitions()
            .map(|d| d.declared_name.as_str())
            .collect::<Vec<_>>(),
        ["Final", "Mid", "i64"]
    );
    let id = proof.alias_definitions().last().unwrap().fingerprint();
    let item = owner
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.fingerprint() == id)
        .unwrap();
    let recipe = db
        .prepared_parse(&owner.module_source().selection().outline.parse)
        .unwrap()
        .type_alias_recipe(
            id,
            &item.header,
            &DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
        )
        .unwrap();
    assert!(proof.matches_query_recipe(&recipe));
    let mut forged = recipe.clone();
    forged.definition = Digest([4; 32]);
    assert!(!proof.matches_query_recipe(&forged));
}
#[test]
fn generic_and_qualified_targets_stay_precise_prerequisites() {
    let (db, owner, scope, key) = current(
        "type A<T>=T;type B=core::primitive::i32;type C=A<i32>;fn main(){}",
        2024,
    );
    assert!(matches!(
        resolve(&db, &owner, &scope, &key, "A"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::GenericSubstitution(_))
    ));
    assert!(matches!(
        resolve(&db, &owner, &scope, &key, "B"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::WrittenTarget(_))
    ));
}
#[test]
fn current_epoch_publication_and_forged_terminal_dependencies_refuse_reuse() {
    let (mut db, owner, scope, key) = current("type A=i32;fn main(){}", 2024);
    let SourceAliasOutcome::Ready(mut proof) = resolve(&db, &owner, &scope, &key, "A") else {
        panic!()
    };
    let stamp = proof.stamps.values_mut().next().unwrap();
    *stamp = Digest([3; 32]);
    assert_eq!(
        proof.revalidate_inputs(&db).unwrap_err().code,
        "LRR-E-SOURCE-ALIAS"
    );
    let SourceAliasOutcome::Ready(proof) = resolve(&db, &owner, &scope, &key, "A") else {
        panic!()
    };
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(
        proof.revalidate_inputs(&db).unwrap_err().code,
        "LRR-E-SOURCE-ALIAS"
    );
    assert!(
        CurrentSourcePrimitiveAlias::capture_original_current(
            &db,
            &owner,
            &scope,
            &key,
            "A",
            &[],
            MAX_BYTES
        )
        .is_err()
    );
}
#[test]
fn metadata_and_foreign_scope_refusal_cannot_attach_a_partial_proof() {
    let (db, owner, scope, key) = current("type A=i32;fn main(){}", 2024);
    let e = CurrentSourcePrimitiveAlias::capture_original_current(
        &db,
        &owner,
        &scope,
        &key,
        "A",
        &[],
        0,
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "LRR-E-METADATA-LIMIT");
    let mut foreign = scope.clone();
    foreign.crate_key = Digest([9; 32]);
    assert!(
        CurrentSourcePrimitiveAlias::capture_original_current(
            &db,
            &owner,
            &foreign,
            &seal_for(&foreign, &key),
            "A",
            &[],
            MAX_BYTES
        )
        .is_err()
    );
    let SourceAliasOutcome::Ready(proof) = resolve(&db, &owner, &scope, &key, "A") else {
        panic!()
    };
    proof.revalidate_inputs(&db).unwrap();
}
#[test]
fn genuine_generated_alias_target_preserves_written_origin_and_source_member() {
    let text = "macro_rules! make {()=>{type Alias=i32;const A:Alias=1;}}make!();fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, scope, key) = current(text, edition);
        let value_key = SealKey {
            domain: SealDomain::Scope {
                id: scope.fingerprint(),
                namespace: Namespace::Value,
            },
            ..key.clone()
        };
        let Some((InputValue::DiscoverySeal(input), _)) =
            db.published_input(&InputKey::DiscoverySeal(value_key))
        else {
            panic!()
        };
        let NameLookup::Defined(a) = input
            .scope
            .as_ref()
            .unwrap()
            .lookup(&BucketKey {
                scope: scope.clone(),
                namespace: Namespace::Value,
                name: "A".into(),
            })
            .unwrap()
        else {
            panic!()
        };
        let parent = CurrentGeneratedConstant::capture(&db, &owner, a.identity, MAX_BYTES).unwrap();
        let crate::frontend::generated_type_admission::GeneratedTypeOutcome::Selected(selected) =
            SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap()
        else {
            panic!("actual selected generated alias")
        };
        let SourceAliasOutcome::Ready(proof) =
            CurrentSourcePrimitiveAlias::capture_generated_current(
                &db,
                &owner,
                &parent,
                &selected,
                &[],
                MAX_BYTES,
            )
            .unwrap()
        else {
            panic!("actual written alias primitive origin")
        };
        assert_eq!(proof.primitive(), PrimitiveType::I32);
        assert_eq!(
            proof.alias_definitions().next().unwrap().declared_name,
            "Alias"
        );
        assert!(proof.steps[0].written.is_some());
        proof.revalidate_inputs(&db).unwrap();
    }
}

#[test]
fn unchanged_generated_generic_alias_keeps_actual_written_source_frontier() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-tt-matchers.rs"
    );
    let (db, owner, scope, key) = current(text, 2024);
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(key))
    else {
        panic!()
    };
    let NameLookup::Defined(alias) = input
        .scope
        .as_ref()
        .unwrap()
        .lookup(&BucketKey {
            scope,
            namespace: Namespace::Type,
            name: "Alias".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(alias.identity))
    else {
        panic!()
    };
    let Some((InputValue::GeneratedSyntax(unit), _)) =
        db.published_input(&InputKey::GeneratedSyntax(parse.content))
    else {
        panic!()
    };
    let header = unit
        .items
        .items()
        .iter()
        .find(|h| h.key.fingerprint() == alias.identity)
        .unwrap();
    let site = WrittenGeneratedType::capture(
        &unit.items,
        &unit.scope,
        &unit.cfg,
        &header.key,
        GeneratedTypeRole::AliasTarget,
        MAX_BYTES,
    )
    .unwrap();
    assert!(matches!(
        site.shape(),
        GeneratedTypeSyntax::GenericOrProjection
    ));
    assert_eq!(
        site.record().key().fingerprint(),
        unit.items.record().key().fingerprint()
    );
    assert!(
        owner
            .module_source()
            .outline()
            .items
            .iter()
            .any(|i| i.def_key.declared_name == "foo")
    );
}
#[test]
fn unchanged_incremental_alias_headers_keep_order_without_running_tool_contracts() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/dep-graph/dep-graph-type-alias.rs"
    );
    let (db, views, _) = prepare(
        vec![input(
            text,
            "dep-graph-original.rs",
            Digest([188; 32]),
            2024,
        )],
        vec![],
    );
    let source = &views[0];
    let parsed = db
        .prepared_parse(&source.selection().outline.parse)
        .unwrap();
    for (name, target) in [("TypeAlias2", "TypeAlias"), ("TypeAlias", "u32")] {
        let item = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap();
        let recipe = parsed
            .type_alias_recipe(
                item.def_key.fingerprint(),
                &item.header,
                &DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                },
            )
            .unwrap();
        assert_eq!(recipe.target.len(), 1);
        assert_eq!(recipe.target[0].text, target);
    }
    // #[rustc_if_this_changed]/query-dep-graph are original tooling assertions, unexecuted.
}

#[test]
fn genuine_original_alias_queries_match_source_headers_and_refuse_stale_reuse() {
    let (mut db, owner, scope, key) = current(FONT, 2024);
    let SourceAliasOutcome::Ready(source) = resolve(&db, &owner, &scope, &key, "FontTableTag")
    else {
        panic!()
    };
    let crate::db::poll::QueryPoll::Ready(Ok(proof)) =
        crate::frontend::source_type_alias_query::QueriedSourcePrimitiveAlias::capture_current(
            &mut db, source, MAX_BYTES,
        )
    else {
        panic!("actual reached source alias query join")
    };
    assert_eq!(proof.query_dependencies().len(), 1);
    assert_eq!(proof.source().primitive(), PrimitiveType::U32);
    assert!(matches!(
        proof.revalidate_current(&mut db),
        crate::db::poll::QueryPoll::Ready(Ok(()))
    ));
    db.begin_expansion_epoch(2).unwrap();
    assert!(matches!(
        proof.revalidate_current(&mut db),
        crate::db::poll::QueryPoll::Ready(Err(_))
    ));
}

#[test]
fn private_payload_tamper_cannot_recover_a_primitive_from_equal_descriptors() {
    let (db, owner, scope, key) = current("type A=i32;fn main(){}", 2024);
    let SourceAliasOutcome::Ready(mut proof) = resolve(&db, &owner, &scope, &key, "A") else {
        panic!()
    };
    proof.primitive = PrimitiveType::U32;
    assert_eq!(
        proof.revalidate_inputs(&db).unwrap_err().code,
        "LRR-E-SOURCE-ALIAS"
    );
}

#[test]
fn unsupported_absent_type_and_actual_long_alias_chain_do_not_default() {
    let (db, owner, scope, key) = current("type A=Unknown;fn main(){}", 2024);
    assert!(matches!(
        resolve(&db, &owner, &scope, &key, "A"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::UnknownPrimitive)
    ));
    let mut text = String::new();
    for i in 0..257 {
        text.push_str(&format!("type A{i}=A{};", i + 1));
    }
    text.push_str("type A257=i32;fn main(){}");
    let (db, owner, scope, key) = current(&text, 2024);
    let e = CurrentSourcePrimitiveAlias::capture_original_current(
        &db,
        &owner,
        &scope,
        &key,
        "A0",
        &[],
        MAX_BYTES,
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "LRR-E-METADATA-LIMIT");
}

#[test]
fn explicit_alias_generic_arguments_keep_the_exact_substitution_prerequisite() {
    let (db, owner, scope, key) = current("type A<T>=T;type B=A<i32>;fn main(){}", 2024);
    assert!(matches!(
        resolve(&db, &owner, &scope, &key, "B"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::GenericSubstitution(_))
    ));
}

#[test]
fn existing_alias_normalizer_consumes_real_type_publication_and_query_join() {
    let (mut db, owner, scope, key) = current(
        "type i64=bool;type A=i64;fn cold(){missing();}fn main(){}",
        2024,
    );
    let graph =
        crate::frontend::names::NameNamespaces::from_outlines(&[owner.module_source().outline()])
            .unwrap();
    let sources = [owner.module_source()];
    let normalizer = crate::frontend::type_aliases::AliasNormalizer::new(&graph, &sources).unwrap();
    let selected = owner
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "A")
        .unwrap()
        .def_key
        .fingerprint();
    let crate::db::poll::QueryPoll::Ready(Ok((
        crate::frontend::type_aliases::AliasTarget::Type(crate::traits::term::TyTerm::Primitive(
            PrimitiveType::Bool,
        )),
        deps,
    ))) = normalizer.normalize_with_dependencies(&mut db, selected)
    else {
        panic!("actual existing alias consumer must preserve source shadow")
    };
    assert!(deps.iter().any(|d|matches!(d,crate::db::dependency::DependencyStamp::Input{key:InputKey::DiscoverySeal(actual),..}if actual==&key)));
    assert!(deps.iter().any(|d| matches!(
        d,
        crate::db::dependency::DependencyStamp::Query {
            key: crate::db::key::QueryKey::TypeAlias(_),
            ..
        }
    )));
    let cert = db
        .source_alias_type_certificate(owner.module_source().selection(), &scope, MAX_BYTES)
        .unwrap()
        .unwrap();
    let InputValue::DiscoverySeal(input) = cert.as_ref() else {
        panic!()
    };
    assert!(
        input
            .compiler_primitive_receipt(owner.module_source().selection())
            .unwrap()
            .is_some()
    );
    assert!(
        input
            .type_prelude_receipt(owner.module_source().selection(), &scope)
            .is_some()
    );
}
#[test]
fn existing_alias_consumer_reports_unprepared_type_source_instead_of_primitive_success() {
    let (mut db, views, _) = prepare(
        vec![input(
            "type A=i32;fn main(){}",
            "unprepared.rs",
            Digest([187; 32]),
            2024,
        )],
        vec![],
    );
    let source = &views[0];
    let graph = crate::frontend::names::NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let sources = [source];
    let normalizer = crate::frontend::type_aliases::AliasNormalizer::new(&graph, &sources).unwrap();
    let initial = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "A")
        .unwrap()
        .def_key
        .fingerprint();
    assert!(matches!(
        normalizer.normalize(&mut db, initial),
        crate::db::poll::QueryPoll::Ready(Ok(
            crate::frontend::type_aliases::AliasTarget::NeedCurrentSource {
                missing: crate::frontend::type_aliases::CurrentAliasPrerequisite::TypeNamespace(_),
                ..
            }
        ))
    ));
}

#[test]
fn genuine_generated_alias_keeps_canonical_member_separate_from_projected_header_identity() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, owner, scope, key) = current(
            "macro_rules! make{()=>{type Word=i64;}}make!();fn main(){}",
            edition,
        );
        let SourceAliasOutcome::Ready(source) = resolve(&db, &owner, &scope, &key, "Word") else {
            panic!("actual original invocation plus generated Type source proof required")
        };
        let canonical = source.selected_definition();
        let projected = source.alias_definitions().next().unwrap().fingerprint();
        assert_ne!(canonical, projected);
        assert_eq!(source.alias_members().collect::<Vec<_>>(), [canonical]);
        let crate::db::poll::QueryPoll::Ready(Ok(join)) =
            crate::frontend::source_type_alias_query::QueriedSourcePrimitiveAlias::capture_current(
                &mut db, source, MAX_BYTES,
            )
        else {
            panic!("canonical generated TypeAlias query must match actual retained recipe")
        };
        assert_eq!(join.source().primitive(), PrimitiveType::I64);
        assert_eq!(
            join.query_dependencies()[0].0,
            crate::db::key::QueryKey::TypeAlias(canonical)
        );
        assert!(
            db.memo(&crate::db::key::QueryKey::TypeAlias(projected))
                .is_none()
        );
    }
}
#[test]
fn original_generated_original_chain_uses_actual_alias_members_and_exact_query_stamps() {
    let (mut db, owner, scope, key) = current(
        "type Start=Middle;type End=i64;macro_rules! make{()=>{type Middle=End;}}make!();fn main(){}",
        2024,
    );
    let SourceAliasOutcome::Ready(source) = resolve(&db, &owner, &scope, &key, "Start") else {
        panic!("actual retained generated hop required")
    };
    assert_eq!(
        source
            .alias_definitions()
            .map(|d| d.declared_name.as_str())
            .collect::<Vec<_>>(),
        ["Start", "Middle", "End"]
    );
    assert!(source.steps[0].written.is_none());
    assert!(source.steps[1].written.is_some());
    assert!(source.steps[2].written.is_none());
    let members = source.alias_members().collect::<Vec<_>>();
    let crate::db::poll::QueryPoll::Ready(Ok(join)) =
        crate::frontend::source_type_alias_query::QueriedSourcePrimitiveAlias::capture_current(
            &mut db, source, MAX_BYTES,
        )
    else {
        panic!("actual three-header query join required")
    };
    assert_eq!(
        join.query_dependencies()
            .iter()
            .map(|(q, _)| q.clone())
            .collect::<Vec<_>>(),
        members
            .into_iter()
            .map(crate::db::key::QueryKey::TypeAlias)
            .collect::<Vec<_>>()
    );
    let cold = owner
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(
        db.memo(&crate::db::key::QueryKey::BodyStructure(cold))
            .is_none()
    );
}
#[test]
fn forged_canonical_member_cannot_reuse_equal_generated_primitive_recipe() {
    let (db, owner, scope, key) = current(
        "macro_rules! make{()=>{type A=i64;type B=i64;}}make!();fn main(){}",
        2024,
    );
    let SourceAliasOutcome::Ready(mut a) = resolve(&db, &owner, &scope, &key, "A") else {
        panic!()
    };
    let SourceAliasOutcome::Ready(b) = resolve(&db, &owner, &scope, &key, "B") else {
        panic!()
    };
    a.steps[0].member = b.selected_definition();
    assert_eq!(
        a.revalidate_inputs(&db).unwrap_err().code,
        "LRR-E-SOURCE-ALIAS"
    );
}
#[test]
fn foreign_original_source_cannot_supply_generated_alias_invocation_owner() {
    let (db, owner, _, _) = current(
        "macro_rules! make{()=>{type A=i64;}}make!();fn main(){}",
        2024,
    );
    let initial = db
        .published_input(&InputKey::DiscoverySeal(SealKey {
            domain: SealDomain::Scope {
                id: owner.module_source().outline().root_scope.fingerprint(),
                namespace: Namespace::Type,
            },
            unit_graph: Digest::of_bytes(b"alias current world"),
            epoch: 1,
        }))
        .unwrap();
    let InputValue::DiscoverySeal(input) = initial.0 else {
        panic!()
    };
    let NameLookup::Defined(a) = input
        .scope
        .as_ref()
        .unwrap()
        .lookup(&BucketKey {
            scope: owner.module_source().outline().root_scope.clone(),
            namespace: Namespace::Type,
            name: "A".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let (_, views, _) = prepare(vec![input_unit_foreign()], vec![]);
    assert!(
        CurrentSourcePrimitiveAlias::original_owner_for_generated(
            &db,
            a.identity,
            &[&views[0]],
            MAX_BYTES
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(
        CurrentSourcePrimitiveAlias::original_owner_for_generated(
            &db,
            a.identity,
            &[owner.module_source()],
            0
        )
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
}
fn input_unit_foreign() -> SourceUnitInput {
    input("fn main(){}", "foreign.rs", Digest([181; 32]), 2024)
}

#[test]
fn opaque_module_macro2_alias_name_cannot_borrow_original_root_identity() {
    let (db, owner, scope, key) = current(
        "#![feature(decl_macro)] macro make(){type A=i32;}make!();fn main(){}",
        2024,
    );
    assert!(matches!(
        resolve(&db, &owner, &scope, &key, "A"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::Origin(
            GeneratedTypePrerequisite::DefinitionSiteSource
        )) | SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(NameLookup::Absent))
    ));
}

#[test]
fn unchanged_visibility_matcher_alias_uses_real_module_and_captured_type_token() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-pub-matcher.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, owner, scope, key) = current_module(original, edition, "with_pub");
        let SourceAliasOutcome::Ready(source) = resolve(&db, &owner, &scope, &key, "H") else {
            panic!("unchanged original module generated alias component")
        };
        assert_eq!(
            source.alias_definitions().next().unwrap().module_path,
            ["with_pub"]
        );
        assert_eq!(source.primitive(), PrimitiveType::I32);
        assert!(source.steps[0].written.is_some());
        let crate::db::poll::QueryPoll::Ready(Ok(join)) =
            crate::frontend::source_type_alias_query::QueriedSourcePrimitiveAlias::capture_current(
                &mut db, source, MAX_BYTES,
            )
        else {
            panic!("genuine original generated-header/query component")
        };
        join.source().revalidate_inputs(&db).unwrap();
        assert!(db.trace.events.iter().all(|e| !matches!(
            e.key,
            crate::db::key::QueryKey::BodyStructure(_) | crate::db::key::QueryKey::Thunk(_)
        )));
    }
    // Original check-pass/compiler contract and unchanged main are not executed here.
}

#[test]
fn actual_empty_custom_catalogue_cannot_replay_standalone_primitive_authority() {
    let (mut db, views, _) = prepare(
        vec![input(
            "type A=i32;fn main(){}",
            "custom-alias.rs",
            Digest([179; 32]),
            2024,
        )],
        vec![],
    );
    let view = &views[0];
    let catalog = crate::frontend::compile_input_catalog::CompileInputCatalog::new(
        crate::facade::unit(&view.selection().outline.parse.profile, "core"),
        2024,
        [],
    )
    .unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest::of_bytes(b"custom alias world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"custom alias environment"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(catalog),
    )
    .unwrap();
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    discovery.publish(&mut db).unwrap();
    let graph = crate::frontend::names::NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [view];
    let normalizer = crate::frontend::type_aliases::AliasNormalizer::new(&graph, &sources).unwrap();
    let initial = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "A")
        .unwrap()
        .def_key
        .fingerprint();
    assert!(matches!(
        normalizer.normalize(&mut db, initial),
        crate::db::poll::QueryPoll::Ready(Ok(
            crate::frontend::type_aliases::AliasTarget::NeedCurrentSource {
                missing: crate::frontend::type_aliases::CurrentAliasPrerequisite::CompilerReceipt,
                ..
            }
        ))
    ));
}

// Every required module Type scope is explicitly prepared and published before
// capture. This helper never manufactures a seal or runs a query-side producer.
fn qualified_world(
    units: Vec<SourceUnitInput>,
    edges: Vec<SourceDependency>,
) -> (Database, Vec<PreparedSourceView>, Vec<SourceDeclarative>) {
    let (mut db, views, mut owners) = prepare(units, edges);
    for owner in &mut owners {
        owner.compiler = Some(Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                owner.profile(),
            )
            .unwrap(),
        ));
    }
    for view in &views {
        for scope in view.outline().scopes.keys().filter(|s| s.owner.is_none()) {
            let compiler = crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &view.selection().outline.parse.profile,
            )
            .unwrap();
            let mut work = SourceDiscovery::new(
                views.iter().map(PreparedSourceView::share).collect(),
                scope.clone(),
                Namespace::Type,
                Digest::of_bytes(b"qualified original alias world"),
                1,
            )
            .unwrap()
            .with_declarative_compiler(
                Digest::of_bytes(b"qualified original alias compile environment"),
                crate::expand::source_declarative::engine_version(),
                Arc::new(compiler),
            )
            .unwrap();
            loop {
                match work.poll().unwrap() {
                    SourceDiscoveryStep::Sealed(_) => break,
                    SourceDiscoveryStep::NeedExpansion { request } => work
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap(),
                    SourceDiscoveryStep::NeedGeneratedExpansion { request } => work
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap(),
                    SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                        "actual qualified Type preparation prerequisite: {} {}",
                        diagnostic.code, diagnostic.message
                    ),
                    _ => panic!(
                        "actual qualified Type preparation requires unadmitted effectful producer"
                    ),
                }
            }
            work.publish(&mut db).unwrap();
        }
    }
    (db, views, owners)
}
fn qualified_key(scope: &ScopeKey) -> SealKey {
    SealKey {
        domain: SealDomain::Scope {
            id: scope.fingerprint(),
            namespace: Namespace::Type,
        },
        unit_graph: Digest::of_bytes(b"qualified original alias world"),
        epoch: 1,
    }
}
fn qualified_alias(db: &Database, owners: &[SourceDeclarative], name: &str) -> SourceAliasOutcome {
    let scope = &owners[0].module_source().outline().root_scope;
    let refs = owners.iter().collect::<Vec<_>>();
    CurrentSourcePrimitiveAlias::capture_original_current(
        db,
        &owners[0],
        scope,
        &qualified_key(scope),
        name,
        &refs,
        MAX_BYTES,
    )
    .unwrap()
}
#[test]
fn written_crate_self_super_alias_chain_joins_target_type_scopes_and_ordered_queries() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = "type Root=i32;type Entry=m::Bridge;mod m{pub type Bridge=self::Deep;pub type Deep=super::Root;}fn cold(){missing();}fn main(){}";
        let (mut db, _, owners) = qualified_world(
            vec![input(
                text,
                "qualified-chain.rs",
                Digest([164; 32]),
                edition,
            )],
            vec![],
        );
        let SourceAliasOutcome::Ready(proof) = qualified_alias(&db, &owners, "Entry") else {
            panic!("actual qualified original alias chain")
        };
        assert_eq!(
            proof
                .alias_definitions()
                .map(|d| d.declared_name.as_str())
                .collect::<Vec<_>>(),
            ["Entry", "Bridge", "Deep", "Root"]
        );
        assert_eq!(proof.primitive(), PrimitiveType::I32);
        let members = proof.alias_members().collect::<Vec<_>>();
        let crate::db::poll::QueryPoll::Ready(Ok(join)) =
            crate::frontend::source_type_alias_query::QueriedSourcePrimitiveAlias::capture_current(
                &mut db, proof, MAX_BYTES,
            )
        else {
            panic!("actual ordered header query join")
        };
        assert_eq!(
            join.query_dependencies()
                .iter()
                .map(|(q, _)| q.clone())
                .collect::<Vec<_>>(),
            members
                .into_iter()
                .map(crate::db::key::QueryKey::TypeAlias)
                .collect::<Vec<_>>()
        );
        assert!(db.trace.events.iter().all(|e| !matches!(
            e.key,
            crate::db::key::QueryKey::BodyStructure(_) | crate::db::key::QueryKey::Thunk(_)
        )));
    }
}
#[test]
fn actual_renamed_extern_alias_keeps_foreign_definition_edition_and_primitive_shadow() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, _, owners) = qualified_world(
            vec![
                input(
                    "extern crate dep as chosen;type Entry=chosen::api::Word;fn main(){}",
                    "caller.rs",
                    Digest([163; 32]),
                    edition,
                ),
                input(
                    "pub mod api{type i64=bool;pub type Word=i64;}",
                    "foreign.rs",
                    Digest([162; 32]),
                    2015,
                ),
            ],
            vec![SourceDependency {
                consumer: Digest([163; 32]),
                alias: "dep".into(),
                provider: Digest([162; 32]),
            }],
        );
        let SourceAliasOutcome::Ready(proof) = qualified_alias(&db, &owners, "Entry") else {
            panic!("actual source-selected foreign aliases")
        };
        assert_eq!(proof.primitive(), PrimitiveType::Bool);
        let defs = proof.alias_definitions().collect::<Vec<_>>();
        assert_eq!(
            defs.iter()
                .map(|d| d.declared_name.as_str())
                .collect::<Vec<_>>(),
            ["Entry", "Word", "i64"]
        );
        assert_eq!(defs[1].crate_key, Digest([162; 32]));
        assert_eq!(proof.steps[1].parse.profile.edition, 2015);
        assert_eq!(proof.steps[0].parse.profile.edition, edition);
        proof.revalidate_inputs(&db).unwrap();
    }
}
#[test]
fn exact_qualified_target_seal_is_required_before_alias_target_header_query() {
    let (db, owner, scope, key) = current(
        "type Entry=m::Word;mod m{pub type Word=i32;}fn main(){}",
        2024,
    );
    let SourceAliasOutcome::Need(SourceAliasPrerequisite::TypeNamespace(target)) =
        resolve(&db, &owner, &scope, &key, "Entry")
    else {
        panic!("actual target Type scope prerequisite")
    };
    assert_eq!(target.module_path, ["m"]);
    assert!(
        db.memo(&crate::db::key::QueryKey::TypeAlias(
            owner
                .module_source()
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.declared_name == "Word")
                .unwrap()
                .def_key
                .fingerprint()
        ))
        .is_none()
    );
}
#[test]
fn qualified_nominal_generic_missing_and_cycle_cases_cannot_become_primitives() {
    let (db, _, owners) = qualified_world(
        vec![input(
            "mod m{pub struct i32;pub type G<T>=T;pub type Cycle=super::Cycle;}type Nominal=m::i32;type Generic=m::G<i32>;type Missing=core::primitive::i32;type Cycle=m::Cycle;fn main(){}",
            "negative-paths.rs",
            Digest([161; 32]),
            2024,
        )],
        vec![],
    );
    assert!(
        matches!(qualified_alias(&db,&owners,"Nominal"),SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(NameLookup::Defined(n)))if owners[0].module_source().outline().items.iter().any(|i|i.def_key.fingerprint()==n.identity&&i.def_key.item_kind==ItemKind::Struct))
    );
    assert!(matches!(
        qualified_alias(&db, &owners, "Generic"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::GenericSubstitution(_))
    ));
    assert!(matches!(
        qualified_alias(&db, &owners, "Missing"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::WrittenTarget(_))
    ));
    let scope = &owners[0].module_source().outline().root_scope;
    let refs = owners.iter().collect::<Vec<_>>();
    let error = CurrentSourcePrimitiveAlias::capture_original_current(
        &db,
        &owners[0],
        scope,
        &qualified_key(scope),
        "Cycle",
        &refs,
        MAX_BYTES,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-TYPE-ALIAS-CYCLE");
}
#[test]
fn qualified_source_consumer_uses_current_source_primitive_join_instead_of_deferred_tokens() {
    let (mut db, views, owners) = qualified_world(
        vec![input(
            "type Entry=crate::m::Word;mod m{pub type Word=i64;}fn main(){}",
            "consumer.rs",
            Digest([160; 32]),
            2024,
        )],
        vec![],
    );
    let graph = crate::frontend::names::NameNamespaces::from_outlines(
        &views.iter().map(|v| v.outline()).collect::<Vec<_>>(),
    )
    .unwrap();
    let refs = views.iter().collect::<Vec<_>>();
    let normalizer = crate::frontend::type_aliases::AliasNormalizer::new(&graph, &refs).unwrap();
    let initial = owners[0]
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Entry")
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(normalizer.normalize_with_dependencies(&mut db,initial),crate::db::poll::QueryPoll::Ready(Ok((crate::frontend::type_aliases::AliasTarget::Type(crate::traits::term::TyTerm::Primitive(PrimitiveType::I64)),deps)))if deps.iter().filter(|d|matches!(d,crate::db::dependency::DependencyStamp::Query{key:crate::db::key::QueryKey::TypeAlias(_),..})).count()>=2)
    );
}
#[test]
fn qualified_budget_and_epoch_refusal_cannot_attach_a_partial_alias_chain() {
    let (mut db, _, owners) = qualified_world(
        vec![input(
            "type Entry=m::Word;mod m{pub type Word=i32;}fn main(){}",
            "budget.rs",
            Digest([159; 32]),
            2024,
        )],
        vec![],
    );
    let SourceAliasOutcome::Ready(proof) = qualified_alias(&db, &owners, "Entry") else {
        panic!()
    };
    let scope = &owners[0].module_source().outline().root_scope;
    let refs = owners.iter().collect::<Vec<_>>();
    assert_eq!(
        CurrentSourcePrimitiveAlias::capture_original_current(
            &db,
            &owners[0],
            scope,
            &qualified_key(scope),
            "Entry",
            &refs,
            0
        )
        .err()
        .unwrap()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(
        proof.revalidate_inputs(&db).unwrap_err().code,
        "LRR-E-SOURCE-ALIAS"
    );
    assert!(
        CurrentSourcePrimitiveAlias::capture_original_current(
            &db,
            &owners[0],
            scope,
            &qualified_key(scope),
            "Entry",
            &refs,
            MAX_BYTES
        )
        .is_err()
    );
}

#[test]
fn unchanged_static_aux_qualified_public_reexport_retains_exact_foreign_alias_header() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/static/static-priv-by-default2.rs"
    );
    let aux = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/static/auxiliary/static_priv_by_default.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, _, owners) = qualified_world(
            vec![
                input(
                    original,
                    "static-priv-by-default2.rs",
                    Digest([158; 32]),
                    edition,
                ),
                input(aux, "static_priv_by_default.rs", Digest([157; 32]), 2015),
            ],
            vec![SourceDependency {
                consumer: Digest([158; 32]),
                alias: "static_priv_by_default".into(),
                provider: Digest([157; 32]),
            }],
        );
        let scope = &owners[0].module_source().outline().root_scope;
        let refs = owners.iter().collect::<Vec<_>>();
        let mut capture = Capture::new(MAX_BYTES).unwrap();
        let path = qualified::WrittenPath {
            segments: ["static_priv_by_default", "bar", "i"]
                .into_iter()
                .map(|n| crate::syntax::imports::ImportSegment::Name(n.into()))
                .collect(),
            absolute: false,
        };
        let Some(NameLookup::Defined(selected)) = qualified::lookup(
            &db,
            &owners[0],
            scope,
            &qualified_key(scope),
            &path,
            &refs,
            &mut capture,
        )
        .unwrap() else {
            panic!("genuine qualified extern reexport component")
        };
        let source_item = owners[1]
            .module_source()
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "reexported_e")
            .unwrap();
        assert_eq!(selected.identity, source_item.def_key.fingerprint());
        assert_eq!(selected.defining_unit, Digest([157; 32]));
        assert_eq!(selected.definition_edition, 2015);
        let mut seed = StableEncoder::new("lrr/test/qualified-original-alias-demand");
        seed.record(&selected);
        seed.record(&qualified_key(scope));
        for s in &path.segments {
            if let crate::syntax::imports::ImportSegment::Name(name) = s {
                seed.string(name)
            }
        }
        let SourceAliasOutcome::Ready(proof) = CurrentSourcePrimitiveAlias::walk_original(
            &db,
            selected,
            seed.finish(),
            &qualified_key(scope),
            &refs,
            capture,
        )
        .unwrap() else {
            panic!("actual source-owned primitive aux alias")
        };
        assert_eq!(proof.primitive(), PrimitiveType::Isize);
        assert_eq!(
            proof.alias_definitions().next().unwrap().module_path,
            ["foo"]
        );
        let crate::db::poll::QueryPoll::Ready(Ok(join)) =
            crate::frontend::source_type_alias_query::QueriedSourcePrimitiveAlias::capture_current(
                &mut db, proof, MAX_BYTES,
            )
        else {
            panic!("exact foreign alias header query component")
        };
        join.source().revalidate_inputs(&db).unwrap();
        // The original expected privacy diagnostics and main are unexecuted.
    }
}
#[test]
fn unchanged_static_aux_private_type_is_inaccessible_not_primitive_absence() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/static/static-priv-by-default2.rs"
    );
    let aux = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/static/auxiliary/static_priv_by_default.rs"
    );
    let (db, _, owners) = qualified_world(
        vec![
            input(original, "privacy-root.rs", Digest([156; 32]), 2015),
            input(aux, "privacy-aux.rs", Digest([155; 32]), 2015),
        ],
        vec![SourceDependency {
            consumer: Digest([156; 32]),
            alias: "static_priv_by_default".into(),
            provider: Digest([155; 32]),
        }],
    );
    let scope = &owners[0].module_source().outline().root_scope;
    let refs = owners.iter().collect::<Vec<_>>();
    let path = qualified::WrittenPath {
        segments: ["static_priv_by_default", "n"]
            .into_iter()
            .map(|n| crate::syntax::imports::ImportSegment::Name(n.into()))
            .collect(),
        absolute: false,
    };
    let mut capture = Capture::new(MAX_BYTES).unwrap();
    assert!(
        matches!(qualified::lookup(&db,&owners[0],scope,&qualified_key(scope),&path,&refs,&mut capture).unwrap(),Some(NameLookup::Inaccessible(paths))if!paths.is_empty())
    );
}
#[test]
fn unchanged_multi_namespace_local_aliases_keep_real_owner_context_and_original_qualified_syntax() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/reexport-from-multiple-namespaces.rs"
    );
    let (mut db, views, _) = prepare(
        vec![input(
            original,
            "multi-ns-original.rs",
            Digest([154; 32]),
            2024,
        )],
        vec![],
    );
    let source = &views[0];
    let graph = crate::frontend::names::NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let refs = [source];
    let normalizer = crate::frontend::type_aliases::AliasNormalizer::new(&graph, &refs).unwrap();
    for name in ["B2", "B3"] {
        let item = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap();
        assert!(item.scope.owner.is_some());
        assert!(
            CurrentSourcePrimitiveAlias::has_qualified_original_target(
                &db,
                item.def_key.fingerprint(),
                &refs
            )
            .unwrap()
        );
        assert!(
            matches!(normalizer.normalize(&mut db,item.def_key.fingerprint()),crate::db::poll::QueryPoll::Ready(Ok(crate::frontend::type_aliases::AliasTarget::NeedOwnerContext(scope)))if scope==item.scope)
        );
    }
    // Type/Value/Macro A reexports and local B2/B3 must not be conflated or promoted into a module scope.
}
#[test]
fn unchanged_ambiguous_glob_exports_keep_two_original_equal_primitive_members() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/issue-107563-ambiguous-glob-reexports.rs"
    );
    let (db, views, _) = prepare(
        vec![input(
            original,
            "ambiguous-original.rs",
            Digest([153; 32]),
            2015,
        )],
        vec![],
    );
    let source = &views[0];
    let mut graph =
        crate::frontend::names::NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let _ = graph.resolve_imports().unwrap();
    let NameLookup::Ambiguous(xs) = graph
        .lookup_unqualified(
            Namespace::Type,
            "X",
            &source.outline().root_scope,
            2015,
            None,
        )
        .unwrap()
    else {
        panic!("original X ambiguity must remain explicit")
    };
    assert_eq!(xs.len(), 2);
    assert_ne!(xs[0].identity, xs[1].identity);
    for selected in xs {
        let item = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.fingerprint() == selected.identity)
            .unwrap();
        assert_eq!(item.def_key.item_kind, ItemKind::TypeAlias);
        let recipe = db
            .prepared_parse(&source.selection().outline.parse)
            .unwrap()
            .type_alias_recipe(
                selected.identity,
                &item.header,
                &DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                },
            )
            .unwrap();
        assert_eq!(recipe.target[0].text, "u8");
    }
    // This is unsealed graph/header ambiguity evidence; original diagnostics are unexecuted.
}
#[test]
fn unchanged_static_method_alias_does_not_grant_associated_or_call_permissions() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/type-alias/static-method-type-alias-11047.rs"
    );
    let (mut db, views, _) = prepare(
        vec![input(
            original,
            "associated-original.rs",
            Digest([152; 32]),
            2024,
        )],
        vec![],
    );
    let source = &views[0];
    let graph = crate::frontend::names::NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let refs = [source];
    let normalizer = crate::frontend::type_aliases::AliasNormalizer::new(&graph, &refs).unwrap();
    let item = source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Ham")
        .unwrap();
    assert!(
        CurrentSourcePrimitiveAlias::has_qualified_original_target(
            &db,
            item.def_key.fingerprint(),
            &refs
        )
        .unwrap()
    );
    assert!(
        matches!(normalizer.normalize(&mut db,item.def_key.fingerprint()),crate::db::poll::QueryPoll::Ready(Ok(crate::frontend::type_aliases::AliasTarget::NeedOwnerContext(scope)))if scope==item.scope)
    );
    assert!(db.trace.events.iter().all(|e| !matches!(
        e.key,
        crate::db::key::QueryKey::BodyStructure(_) | crate::db::key::QueryKey::Thunk(_)
    )));
}

#[test]
fn completed_generated_record_is_not_silently_removed_from_qualified_original_replay() {
    let (db, owner, scope, key) = current(
        "type Entry=m::Word;mod m{pub type Word=i32;}macro_rules! empty{()=>{}}empty!();fn main(){}",
        2024,
    );
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(key.clone()))
    else {
        panic!()
    };
    assert!(!input.records.is_empty());
    assert!(matches!(
        resolve(&db, &owner, &scope, &key, "Entry"),
        SourceAliasOutcome::Need(SourceAliasPrerequisite::WrittenTarget(_))
    ));
}
