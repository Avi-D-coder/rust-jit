//! M017 retained attribute frontier versus function-like declarative fallback.
//! Stock negative cases are discovery components, not full rustc diagnostic passes.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn discovery(text: &str, edition: u16) -> (SourceDiscovery, PreparedSourceInputs) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let input = SourceUnitInput {
        path: SourcePath::new("main.rs").unwrap(),
        crate_key: Digest([241; 32]),
        module_path: vec![],
        profile: profile.clone(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
        path: input.path.clone(),
        outline: lrr::db::key::OutlineKey {
            parse: ParseKey {
                content: input.blob.content(),
                profile: profile.clone(),
            },
            crate_key: input.crate_key,
            module_path: vec![],
            cfg: input.cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(None, vec![input], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest([242; 32]),
        1,
    )
    .unwrap()
    .with_declarative(
        Digest([243; 32]),
        Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
    )
    .unwrap()
    .with_compiler(std::sync::Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile).unwrap(),
    ))
    .unwrap();
    (work, inputs)
}
#[test]
fn cold_attributes_do_not_enter_function_like_macro_fallback() {
    for edition in [2015, 2018, 2021, 2024] {
        for attribute in [
            "unknown",
            "foreign::allow(dead_code)",
            "::allow(dead_code)",
            "derive(Clone)",
        ] {
            // Exact existing milestone minimal witness, with explicit owning profiles.
            let text = format!("struct Global;fn cold(){{#[{attribute}] impl Global{{}}}}");
            let (mut work, inputs) = discovery(&text, edition);
            assert!(
                matches!(work.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
                "{attribute} in edition {edition}"
            );
            assert!(
                !work
                    .impl_header_inventory()
                    .unwrap()
                    .pending_producers()
                    .is_empty()
            );
            assert_eq!(work.accounted_expansion_count().unwrap(), 0);
            assert!(work.sealed_expansions().is_err());
            let mut db = lrr::db::memo::Database::default();
            db.trace.detailed = true;
            inputs.publish(&mut db).unwrap();
            let revision = db.revision();
            assert!(
                work.publish(&mut db).is_err(),
                "unresolved attribute cannot publish a complete universe"
            );
            assert_eq!(db.revision(), revision);
            assert!(db.trace.events.is_empty());
        }
    }
}
#[test]
fn unchanged_rustc_unknown_attributes_remain_unpublished_discovery_frontiers() {
    for text in [
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/attributes/attr_unknown_custom_attr.rs"
        ),
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/attributes/unknown-attr.rs"),
    ] {
        let (mut work, _) = discovery(text, 2015);
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
        assert_eq!(work.accounted_expansion_count().unwrap(), 0);
        assert!(work.sealed_expansions().is_err());
    }
}
#[test]
fn qualified_function_like_macro_still_expands_its_actual_declarations() {
    let text = "#[macro_export] macro_rules! make {()=>{impl Global{}}} struct Global;fn cold(){crate::make!();}";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut work, inputs) = discovery(text, edition);
        let mut completed = false;
        for _ in 0..100 {
            match work.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => work
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => work
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Sealed(_) => {
                    completed = true;
                    break;
                }
                _ => panic!("function-like macro frontier differs"),
            }
        }
        assert!(completed);
        assert_eq!(work.accounted_expansion_count().unwrap(), 1);
        assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
        let mut db = lrr::db::memo::Database::default();
        inputs.publish(&mut db).unwrap();
        work.publish(&mut db).unwrap();
    }
}
