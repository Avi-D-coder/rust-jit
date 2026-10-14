//! Real source/expansion/header components only; no evaluator, layout or main execution.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    expand::{
        generated_items::GeneratedItems,
        source_declarative::SourceDeclarativeDemand,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::generated_macros::GeneratedMacroCatalog,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{generated_type_site::GeneratedTypeRole, parse::ParseKey},
};
use std::sync::Arc;
const VIS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-pub-matcher.rs"
);
const RAW: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/parser/raw/raw-idents.rs");
const MODERN: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/dollar-crate-modern.rs"
);
const INTERCRATE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/auxiliary/intercrate.rs"
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
fn component(
    text: &str,
    edition: u16,
    chosen: &str,
) -> (SourceDeclarative, GeneratedItems, GeneratedMacroCatalog) {
    let (_, views, providers) = prepare(
        vec![input(text, "type-original.rs", Digest([171; 32]), edition)],
        vec![],
    );
    let call = views[0]
        .outline()
        .item_invocations
        .iter()
        .find(|c| c.range.start as usize == text.find(chosen).unwrap())
        .unwrap();
    let routes = providers
        .iter()
        .flat_map(|p| p.routes_in(&providers).unwrap())
        .collect::<Vec<_>>();
    let graph =
        crate::frontend::source_macro_routes::SourceMacroRoutes::discovery_graph(&routes).unwrap();
    let SourceDeclarativeDemand::Ready(request) = providers[0]
        .demand_for_discovery(
            call.item_input,
            Digest::of_bytes(b"type component compile env"),
            crate::expand::source_declarative::engine_version(),
            &graph,
            &providers,
        )
        .unwrap()
    else {
        panic!("actual original type expansion prerequisite")
    };
    let items =
        GeneratedItems::admit(request.key(), Arc::new(request.expand().unwrap()), 1).unwrap();
    let catalog = GeneratedMacroCatalog::new(
        &items,
        call.scope.clone(),
        Key(items.record().key().parent_context.0),
        views[0].captured_macro_cfg().unwrap(),
        1,
    )
    .unwrap();
    (providers.into_iter().next().unwrap(), items, catalog)
}
fn world(
    text: &str,
    edition: u16,
    module: &[&str],
) -> (Database, SourceDeclarative, CurrentGeneratedConstant) {
    let (mut db, views, providers) = prepare(
        vec![input(text, "type-current.rs", Digest([172; 32]), edition)],
        vec![],
    );
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
    let owner = providers.into_iter().next().unwrap();
    let parent =
        CurrentGeneratedConstant::capture(&db, &owner, definition.unwrap(), MAX_BYTES).unwrap();
    (db, owner, parent)
}
#[test]
fn unchanged_raw_generated_unit_and_alias_sites_keep_all_four_origin_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (_, items, catalog) = component(RAW, edition, "tests!(r#break);");
        let (_, scope, _) = catalog.captured_owner();
        let constant = items
            .items()
            .iter()
            .find(|h| h.kind == crate::expand::generated_items::GeneratedHeaderKind::Const)
            .unwrap();
        let site = WrittenGeneratedType::capture(
            &items,
            scope,
            catalog.cfg(),
            &constant.key,
            GeneratedTypeRole::Declaration,
            MAX_BYTES,
        )
        .unwrap();
        assert!(matches!(site.shape(), GeneratedTypeSyntax::Unit));
        assert_eq!(site.record().key().call_edition, edition);
        assert_eq!(site.record().key().definition_edition, edition);
        let alias = items
            .items()
            .iter()
            .find(|h| {
                h.kind == crate::expand::generated_items::GeneratedHeaderKind::Type
                    && items.text()[h.start as usize..h.end as usize].contains("=")
            })
            .unwrap();
        let site = WrittenGeneratedType::capture(
            &items,
            scope,
            catalog.cfg(),
            &alias.key,
            GeneratedTypeRole::AliasTarget,
            MAX_BYTES,
        )
        .unwrap();
        assert!(matches!(site.shape(), GeneratedTypeSyntax::Unit));
        assert_eq!(site.scope().module_path.len(), 2);
    }
}
#[test]
fn unchanged_vis_alias_target_is_origin_data_never_a_primitive_spelling_grant() {
    for edition in [2015, 2018, 2021, 2024] {
        let (_, items, catalog) = component(VIS, edition, "vis_passthru! { pub type H = i32; }");
        let (_, scope, _) = catalog.captured_owner();
        let alias = items
            .items()
            .iter()
            .find(|h| h.kind == crate::expand::generated_items::GeneratedHeaderKind::Type)
            .unwrap();
        let site = WrittenGeneratedType::capture(
            &items,
            scope,
            catalog.cfg(),
            &alias.key,
            GeneratedTypeRole::AliasTarget,
            MAX_BYTES,
        )
        .unwrap();
        let GeneratedTypeSyntax::Path(parts) = site.shape() else {
            panic!()
        };
        assert!(matches!(parts[0].kind(),GeneratedTypeSegmentKind::Name(n) if n=="i32"));
        assert_eq!(
            parts[0].span().edition,
            crate::expand::source_declarative::edition(edition).unwrap()
        );
        assert_eq!(site.role(), GeneratedTypeRole::AliasTarget);
    }
}
#[test]
fn unchanged_scalar_type_absence_keeps_explicit_primitive_environment_frontier() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, parent) = world(VIS, edition, &["with_pub"]);
        assert!(matches!(
            SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap(),
            GeneratedTypeOutcome::Need(GeneratedTypePrerequisite::PrimitiveEnvironment)
        ));
    }
}
#[test]
fn generated_alias_unit_joins_exact_header_without_original_source_fabrication() {
    let text = "macro_rules!m{()=>{type U=();const A:U=();}}m!();fn main(){}";
    let (db, owner, parent) = world(text, 2024, &[]);
    let GeneratedTypeOutcome::Selected(selected) =
        SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap()
    else {
        panic!()
    };
    let SelectedSourceType::AliasUnit(alias) = selected.result() else {
        panic!()
    };
    assert_ne!(alias.identity, parent.retained().member().fingerprint());
    assert_eq!(selected.unit_primitive(), Some(PrimitiveType::Unit));
    assert!(selected.alias_site.is_some());
    assert!(
        selected
            .dependencies()
            .contains_key(&InputKey::DefinitionHeader(alias.identity))
    );
    selected.revalidate(&db, &owner, &parent, &[]).unwrap();
}
#[test]
fn original_alias_unit_retains_member_identity_and_genuine_cached_header() {
    let text = "type U=();macro_rules!m{()=>{const A:U=();}}m!();fn main(){}";
    let (db, owner, parent) = world(text, 2024, &[]);
    let GeneratedTypeOutcome::Selected(selected) =
        SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap()
    else {
        panic!()
    };
    let SelectedSourceType::AliasUnit(alias) = selected.result() else {
        panic!()
    };
    let actual = owner
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "U")
        .unwrap();
    assert_eq!(alias.identity, actual.def_key.fingerprint());
    assert!(selected.alias_site.is_none());
    assert_eq!(selected.unit_primitive(), Some(PrimitiveType::Unit));
}
#[test]
fn primitive_shadow_and_nonunit_alias_never_collapse_to_a_descriptor() {
    for (text, nominal) in [
        (
            "struct i32;macro_rules!m{()=>{const A:i32=i32;}}m!();fn main(){}",
            true,
        ),
        (
            "type i32=bool;macro_rules!m{()=>{const A:i32=true;}}m!();fn main(){}",
            false,
        ),
    ] {
        let (db, owner, parent) = world(text, 2024, &[]);
        let GeneratedTypeOutcome::Selected(selected) =
            SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap()
        else {
            panic!()
        };
        assert_eq!(selected.unit_primitive(), None);
        if nominal {
            assert!(matches!(
                selected.result(),
                SelectedSourceType::NominalHeader(_)
            ));
        } else {
            assert!(matches!(
                selected.result(),
                SelectedSourceType::AliasHeader(_)
            ));
            assert_eq!(
                selected.target_prerequisite(),
                Some(GeneratedTypePrerequisite::AliasTargetHygiene)
            );
        }
    }
}
#[test]
fn generated_generic_alias_and_absolute_path_keep_exact_prerequisites() {
    let (db, owner, parent) = world(
        "macro_rules!m{()=>{type U<T>=();const A:U=();}}m!();fn main(){}",
        2024,
        &[],
    );
    assert!(matches!(
        SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap(),
        GeneratedTypeOutcome::Need(GeneratedTypePrerequisite::GenericOrAssociated)
    ));
    let (db, owner, parent) = world(
        "type U=();macro_rules!m{()=>{const A: ::U = ();}}m!();fn main(){}",
        2024,
        &[],
    );
    assert!(matches!(
        SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap(),
        GeneratedTypeOutcome::Need(GeneratedTypePrerequisite::QualifiedModuleRoute)
    ));
}
#[test]
fn actual_epoch_parent_and_quota_refusal_precede_source_type_mint() {
    let (mut db, owner, parent) = world(
        "macro_rules!m{()=>{const A:()=();}}m!();fn main(){}",
        2024,
        &[],
    );
    assert!(SelectedGeneratedType::capture_current(&db, &owner, &parent, 0).is_err());
    let GeneratedTypeOutcome::Selected(selected) =
        SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).unwrap()
    else {
        panic!()
    };
    db.begin_expansion_epoch(2).unwrap();
    assert!(selected.revalidate(&db, &owner, &parent, &[]).is_err());
    assert!(SelectedGeneratedType::capture_current(&db, &owner, &parent, MAX_BYTES).is_err());
}
#[test]
fn unchanged_cross_edition_dollar_crate_retains_actual_selected_mark_and_record() {
    for (call, definition) in [(2015, 2024), (2018, 2021), (2021, 2018), (2024, 2015)] {
        let root = Digest([173; 32]);
        let aux = Digest([174; 32]);
        let (_, views, providers) = prepare(
            vec![
                input(MODERN, "modern.rs", root, call),
                input(INTERCRATE, "intercrate.rs", aux, definition),
            ],
            vec![SourceDependency {
                consumer: root,
                alias: "intercrate".into(),
                provider: aux,
            }],
        );
        let call_site=views[0].outline().item_invocations.iter().find(|c| views[0].macro_call_at(c.range).unwrap().segments().last().is_some_and(|s|matches!(s,crate::syntax::imports::ImportSegment::Name(n) if n=="uses_dollar_crate_modern"))).unwrap();
        let routes = providers
            .iter()
            .flat_map(|p| p.routes_in(&providers).unwrap())
            .collect::<Vec<_>>();
        let graph =
            crate::frontend::source_macro_routes::SourceMacroRoutes::discovery_graph(&routes)
                .unwrap();
        let SourceDeclarativeDemand::Ready(request) = providers[0]
            .demand_for_discovery(
                call_site.item_input,
                Digest::of_bytes(b"cross type component compile env"),
                crate::expand::source_declarative::engine_version(),
                &graph,
                &providers,
            )
            .unwrap()
        else {
            panic!()
        };
        let items =
            GeneratedItems::admit(request.key(), Arc::new(request.expand().unwrap()), 1).unwrap();
        let member = items
            .items()
            .iter()
            .find(|h| h.kind == crate::expand::generated_items::GeneratedHeaderKind::Type)
            .unwrap();
        let site = WrittenGeneratedType::capture(
            &items,
            &call_site.scope,
            views[0].captured_macro_cfg().unwrap(),
            &member.key,
            GeneratedTypeRole::AliasTarget,
            MAX_BYTES,
        )
        .unwrap();
        let GeneratedTypeSyntax::Path(parts) = site.shape() else {
            panic!()
        };
        assert!(matches!(
            parts[0].kind(),
            GeneratedTypeSegmentKind::Keyword(TypeKeyword::DollarCrate)
        ));
        let facts = items.hygiene_facts();
        let origin = facts.dollar_crate_origin(parts[0].span().context).unwrap();
        assert_eq!(origin.unit(), Key(aux.0));
        assert_eq!(origin.expansion(), Key(request.key().fingerprint().0));
        assert_eq!(
            facts.context(origin.mark()).unwrap().expansion,
            Some(origin.expansion())
        );
        assert_eq!(site.record().key().call_edition, call);
        assert_eq!(site.record().key().definition_edition, definition);
        assert!(facts.dollar_crate_origin(Key([175; 32])).is_err());
        // Component provenance is not a completed provider Type seal or alias result.
    }
}
#[test]
fn unchanged_nominal_primitive_shadow_and_local_macro_scope_are_not_spelling_authority() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/shadowed/primitive-type-shadowing.rs"
    );
    let (_, views, _) = prepare(
        vec![input(text, "primitive-shadow.rs", Digest([176; 32]), 2024)],
        vec![],
    );
    let nominal = views[0]
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "bool" && i.def_key.item_kind == ItemKind::Struct)
        .unwrap();
    assert_eq!(nominal.scope.crate_key, Digest([176; 32]));
    assert_ne!(nominal.header.fingerprint(), nominal.def_key.fingerprint());
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/generate-mod.rs"
    );
    let (_, views, _) = prepare(
        vec![input(text, "generate-mod.rs", Digest([177; 32]), 2024)],
        vec![],
    );
    let parsed = crate::syntax::parse::ParsedFile::parse(
        views[0].captured_macro_blob().unwrap(),
        views[0].outline().profile.clone(),
    )
    .unwrap();
    let selected = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .find(|c| {
            c.path()
                .is_some_and(|p| p.syntax().text().to_string() == "genmod")
        })
        .unwrap();
    let call = views[0]
        .macro_call_at(crate::syntax::ast_adapter::range(
            selected.syntax().text_range(),
        ))
        .unwrap();
    assert!(call.requires_body_entry());
    assert!(
        !views[0]
            .outline()
            .item_invocations
            .iter()
            .any(|c| c.range == call.range())
    );
}
