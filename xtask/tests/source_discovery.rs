use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::names::NameLookup,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> PreparedSourceView {
    source_inputs(text).0
}
fn source_inputs(text: &str) -> (PreparedSourceView, PreparedSourceInputs) {
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
fn discover(view: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        Digest([242; 32]),
        1,
    )
    .unwrap()
}
fn discover_source_derives(view: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest([242; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
    .with_compiler(std::sync::Arc::new(
        lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &ProfileIdentity::current(),
        )
        .unwrap(),
    ))
    .unwrap()
}
#[test]
fn rustc_original_empty_derive_retires_source_frontier_without_expansion_record() {
    let (v, inputs) = source_inputs(include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/derives/deriving-meta-empty-trait-list.rs"
    ));
    let mut w = discover_source_derives(&v);
    assert!(w.lookup("_Bar").is_err());
    let SourceDiscoveryStep::Sealed(_) = w.poll().unwrap() else {
        panic!("compiler-owned empty source derive did not settle")
    };
    assert!(matches!(w.lookup("_Bar").unwrap(), NameLookup::Defined(_)));
    assert_eq!(w.accounted_expansion_count().unwrap(), 0);
    assert_eq!(w.sealed_expansions().unwrap().len(), 0);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(
            &lrr::db::provider::SyntaxProvider,
            lrr::db::key::QueryKey::DiscoverySeal(w.key().clone())
        ),
        lrr::db::poll::QueryPoll::Ready(Ok(_))
    ));
    let observation = serde_json::json!({
        "upstream_case": "tests/ui/derives/deriving-meta-empty-trait-list.rs",
        "source_content": v.selection().outline.parse.content.hex(),
        "seal_key": w.key().fingerprint().hex(),
        "accounted_expansions": w.accounted_expansion_count().unwrap(),
        "expansion_records": w.sealed_expansions().unwrap().len(),
        "published": true,
        "full_fixture_acceptance": false,
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("artifacts/macos-first/m013-source-empty-observation.json");
    std::fs::write(path, serde_json::to_vec_pretty(&observation).unwrap()).unwrap();
}
#[test]
fn original_empty_derives_preserve_cfg_members_and_repeated_annotations() {
    for text in [
        "#[derive()] #[cfg_attr(all(),derive())] #[repr(C)] pub struct Value(#[doc=\"kept\"] pub u8, #[cfg(any())] #[unknown] Missing);",
        "#[derive()] pub enum Value { #[cfg(any())] #[unknown] Gone(Missing), #[doc=\"kept\"] Kept }",
        "use core::derive as vacant; #[vacant()] pub union Value { pub kept: u8 }",
    ] {
        let v = source(text);
        let mut w = discover_source_derives(&v);
        assert!(
            matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)),
            "{text}"
        );
        assert!(matches!(w.lookup("Value").unwrap(), NameLookup::Defined(_)));
    }
}
#[test]
fn original_empty_derive_does_not_hide_unknown_attributes_or_nonempty_groups() {
    for text in [
        "#[derive()] #[unknown] pub struct Value;",
        "#[derive()] pub struct Value(#[unknown] u8);",
        "#[derive()] #[derive(Clone)] pub struct Value;",
        "#[unknown()] pub struct Value;",
        "#[derive] pub struct Value;",
        "use core::test as vacant; #[vacant()] pub struct Value;",
        "macro_rules! vacant { () => {} } #[vacant()] pub struct Value;",
        "#[derive()] pub struct Value(#[foreign::doc=\"unknown\"] u8);",
    ] {
        let v = source(text);
        let mut w = discover_source_derives(&v);
        match w.poll() {
            Ok(SourceDiscoveryStep::Incomplete { diagnostic, .. }) => {
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE")
            }
            Err(error) if text.contains("#[derive]") => {
                assert_eq!(error.code, "LRR-E-MACRO-OUTPUT")
            }
            _ => panic!("unresolved source transformation settled: {text}"),
        }
        assert!(w.lookup("Value").is_err());
    }
    let v = source("#[derive()] pub struct Value;");
    assert!(matches!(
        discover(&v).poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
}
#[test]
fn original_empty_derive_seal_suspends_after_captured_source_changes() {
    use lrr::db::{
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    };
    for changed in [
        "#[derive(Clone)] pub struct Value;",
        "#[derive()] pub struct Value(#[unknown] u8);",
        "#[derive()] pub struct Value; fn cold(){let _:Missing=missing();}",
    ] {
        let (v, inputs) = source_inputs("#[derive()] pub struct Value;");
        let mut w = discover_source_derives(&v);
        assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        w.publish(&mut db).unwrap();
        let key = QueryKey::DiscoverySeal(w.key().clone());
        assert!(matches!(
            db.fetch(&SyntaxProvider, key.clone()),
            QueryPoll::Ready(Ok(_))
        ));
        source_inputs(changed).1.publish(&mut db).unwrap();
        assert!(matches!(
            db.fetch(&SyntaxProvider, key),
            QueryPoll::NeedInput(InputRequest::Discover { .. })
        ));
    }
}
#[test]
fn captured_source_import_cycle_with_origin_seals_and_preserves_definition_identity() {
    let v = source(
        "mod a{pub use crate::b::*;pub fn chosen()->i64{3_i64}}mod b{pub use crate::a::*;}pub use b::chosen as selected;fn cold(){let _:Missing=missing();}",
    );
    let mut w = discover(&v);
    assert!(w.lookup("selected").is_err());
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("origin lost")
    };
    assert_eq!(
        d.identity,
        v.outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "chosen")
            .unwrap()
            .def_key
            .fingerprint()
    );
}
#[test]
fn captured_unknown_item_macro_blocks_discovery_instead_of_sealing_missing_names() {
    let v = source("unknown!();pub fn visible(){}use generated::chosen;");
    let mut w = discover(&v);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = w.poll().unwrap()
    else {
        panic!("unknown producer sealed")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(w.lookup("visible").is_err());
}
#[test]
fn captured_cfg_disabled_producer_does_not_block_and_cold_body_is_not_lowered() {
    let v =
        source("#[cfg(any())] unknown!();pub fn visible(){}fn cold(){let _:Missing=missing();}");
    let mut w = discover(&v);
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert!(matches!(
        w.lookup("visible").unwrap(),
        NameLookup::Defined(_)
    ));
    assert_eq!(w.lookup("absent").unwrap(), NameLookup::Absent);
}
#[test]
fn captured_source_seal_depends_on_scanned_content_even_without_selected_name_changes() {
    let mut seals = vec![];
    for text in [
        "pub fn selected(){}fn unused(){1;}",
        "pub fn selected(){}fn unused(){2;}",
    ] {
        let v = source(text);
        let mut w = discover(&v);
        let SourceDiscoveryStep::Sealed(s) = w.poll().unwrap() else {
            panic!("seal absent")
        };
        seals.push(s.fingerprint());
        assert!(matches!(
            w.lookup("selected").unwrap(),
            NameLookup::Defined(_)
        ));
    }
    assert_ne!(seals[0], seals[1]);
}

#[test]
fn cold_macro_requires_real_expansion_before_source_scope_completion() {
    let view = source(
        "trait Mark{fn mark(&self)->i32;}struct Global;macro_rules! make{()=>{{impl Mark for Global{fn mark(&self)->i32{7}}7}}}fn cold(){let x=make!();let wrong:i32=\"cold\";missing();}fn main(){}",
    );
    let mut work = discover(&view)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("cold declaration producer was omitted before the source scope completed")
    };
    assert_eq!(
        request.key().grammar,
        lrr::expand::identity::GrammarSlot::Expression
    );
    let SourceDiscoveryStep::NeedExpansion { request: repeat } = work.poll().unwrap() else {
        panic!("cold pending producer disappeared")
    };
    assert_eq!(request.key(), repeat.key());
    let record = std::sync::Arc::new(request.expand().unwrap());
    assert!(record.token_count() > 0);
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.sealed_declaration_expansions().unwrap().len(), 1);
}

#[test]
fn rustc_cold_statement_discovery_adopts_original_nested_output_before_seal() {
    let view = source(include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    ));
    let mut work = discover(&view)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    let mut keys = Vec::new();
    loop {
        assert!(keys.len() <= 2, "unexpected original fixture producers");
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                keys.push(request.key().clone());
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => break,
            _ => panic!("original declaration fixture remained incomplete"),
        }
    }
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
    assert_eq!(work.sealed_declaration_expansions().unwrap().len(), 2);
    assert!(
        work.sealed_expansions().unwrap().len() == 0,
        "expression/statement records became item facets"
    );
}

#[test]
fn cold_unknown_child_keeps_source_scope_and_declaration_records_unpublished() {
    let view = source("macro_rules! make{()=>{{let x=unknown!();7}}}fn cold(){let x=make!();}");
    let mut work = discover(&view)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("root absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_declaration_expansions().is_err());
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
}

#[test]
fn cold_macro_waits_for_unknown_outer_annotation_in_original_owner() {
    let view = source(
        "macro_rules! make{()=>{{impl Global{}7}}}#[unknown]struct Global;fn cold(){let x=make!();}",
    );
    let mut work = discover(&view)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
}

#[test]
fn cold_macro_after_empty_derive_uses_original_owner_readiness() {
    let view = source(
        "macro_rules! make{()=>{{impl Global{}7}}}#[derive()]struct Global;fn cold(){let x=make!();}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("settled derive prevented cold demand")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
}

#[test]
fn cold_declaration_foreign_reply_preserves_pending_work_and_budget() {
    let view = source("macro_rules! make{()=>{7}}fn cold(){let x=make!();}");
    let mut work = discover(&view)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("demand absent")
    };
    let foreign = source("macro_rules! other{()=>{9}}fn cold(){let x=other!();}");
    let mut other = discover(&foreign)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request: foreign } = other.poll().unwrap() else {
        panic!("foreign demand absent")
    };
    assert_eq!(
        work.admit_expansion(std::sync::Arc::new(foreign.expand().unwrap()))
            .unwrap_err()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
    let SourceDiscoveryStep::NeedExpansion { request: repeat } = work.poll().unwrap() else {
        panic!("foreign reply completed work")
    };
    assert_eq!(request.key(), repeat.key());
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
}

#[test]
fn cold_runtime_intrinsic_no_item_proof_settles_without_running_predicate() {
    let view = source("fn cold(){assert!(missing());let wrong:i32=\"cold\";}fn main(){}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    assert!(work.impl_header_inventory().unwrap().headers().is_empty());
}

#[test]
fn cold_runtime_intrinsic_with_nested_producer_cannot_use_no_item_proof() {
    let view = source("fn cold(){assert!({let x=unknown!();true});}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
}

#[test]
fn cold_runtime_intrinsic_name_shadow_expands_actual_user_definition() {
    let view = source(
        "struct Global;macro_rules! assert{($e:expr)=>{{impl Global{}7}}}fn cold(){let x=assert!(true);}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("user assert treated as intrinsic")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
}

#[test]
fn cold_runtime_proof_publication_checks_current_captured_source() {
    let (view, inputs) = source_inputs("fn cold(){assert!(missing());}fn main(){}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let (_, changed) = source_inputs("fn cold(){assert!(other());}fn main(){}");
    changed.publish(&mut db).unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn cold_declaration_scope_publication_preserves_exact_non_item_record() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! make{()=>{{impl Global{}7}}}fn cold(){let x=make!();}",
    );
    let mut work = discover(&view)
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("cold demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    work.admit_expansion(record.clone()).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = lrr::db::dependency::InputKey::ExpansionRecord(record.key().fingerprint());
    assert!(
        matches!(db.input(&key).as_deref(),Some(lrr::db::dependency::InputValue::ExpansionRecord(published)) if published.descriptor().fingerprint()==record.descriptor().fingerprint())
    );
}

#[test]
fn cold_audited_intrinsic_discovers_actual_impl_argument_without_execution() {
    let view = source(
        "struct Global;fn cold(){assert!({impl Global{fn marker(&self)->i32{7}}let wrong:i32=\"cold\";missing();true});}",
    );
    let mut work = discover_source_derives(&view);
    assert!(
        matches!(work.poll().unwrap(), SourceDiscoveryStep::Sealed(_)),
        "audited argument declarations remained undiscovered"
    );
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    assert!(work.sealed_declaration_expansions().unwrap().is_empty());
}

#[test]
fn cold_intrinsic_argument_publication_rejects_changed_source() {
    let text = "struct Global;fn cold(){assert!({impl Global{}missing();true});}fn main(){}";
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    let (_, changed) =
        source_inputs("struct Global;fn cold(){assert!({impl Global{}other();true});}fn main(){}");
    changed.publish(&mut db).unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn cold_intrinsic_nonblock_expression_preserves_delimiters_and_pending_macro() {
    let (view, inputs) = source_inputs(
        "struct Global;fn cold(){assert!(({impl Global{}true}) && missing());}fn main(){}",
    );
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let nested = source("fn cold(){assert!(({unknown!();true}) && missing());}");
    let mut pending = discover_source_derives(&nested);
    assert!(matches!(
        pending.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(pending.accounted_expansion_count().unwrap(), 0);
}

#[test]
fn cold_intrinsic_nested_macro_demands_real_reply_before_impl_publication() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! make{()=>{{impl Global{}true}}}fn cold(){assert!(make!());}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("nested intrinsic argument did not demand actual macro expansion")
    };
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    assert!(work.impl_header_inventory().unwrap().headers().is_empty());
    let record = std::sync::Arc::new(request.expand().unwrap());
    work.admit_expansion(record.clone()).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    assert_eq!(work.sealed_declaration_expansions().unwrap().len(), 1);
    assert_eq!(
        work.sealed_declaration_expansions().unwrap()[0]
            .descriptor()
            .fingerprint(),
        record.descriptor().fingerprint()
    );
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = lrr::db::dependency::InputKey::ExpansionRecord(record.key().fingerprint());
    assert!(
        matches!(db.input(&key).as_deref(),Some(lrr::db::dependency::InputValue::ExpansionRecord(found)) if found.descriptor().fingerprint()==record.descriptor().fingerprint())
    );
    let (_, changed) = source_inputs(
        "struct Global;macro_rules! make{()=>{{impl Global{}false}}}fn cold(){assert!(make!());}",
    );
    changed.publish(&mut db).unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn cold_intrinsic_multiple_argument_producers_preserve_scope_and_exact_budget() {
    let text = "struct Global;fn cold(){assert_eq!({macro_rules! local{()=>{{impl Global{}7}}}local!()}, {macro_rules! local{()=>{{impl Global{fn second(){}}7}}}local!()});}";
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    for count in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("argument producer skipped")
        };
        assert_eq!(work.accounted_expansion_count().unwrap(), count);
        work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 2);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}
#[test]
fn cold_intrinsic_foreign_reply_and_unknown_child_preserve_unfinished_frontier() {
    let view = source(
        "struct Global;macro_rules! make{()=>{{impl Global{}unknown!();true}}}fn cold(){assert!(make!());}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("argument demand absent")
    };
    let foreign = source("macro_rules! other{()=>{true}}fn cold(){assert!(other!());}");
    let mut other = discover_source_derives(&foreign);
    let SourceDiscoveryStep::NeedExpansion {
        request: other_request,
    } = other.poll().unwrap()
    else {
        panic!("foreign demand absent")
    };
    assert!(
        work.admit_expansion(std::sync::Arc::new(other_request.expand().unwrap()))
            .is_err()
    );
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    assert!(work.sealed_declaration_expansions().is_err());
}

#[test]
fn cold_intrinsic_generated_child_keeps_actual_ancestry_and_parent_scope() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! leaf{()=>{{impl Global{}true}}}macro_rules! make{()=>{{leaf!()}}}fn cold(){assert!(make!());}",
    );
    let mut work = discover_source_derives(&view);
    for count in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("argument subtree demand absent")
        };
        assert_eq!(work.accounted_expansion_count().unwrap(), count);
        work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}
#[test]
fn cold_intrinsic_argument_parent_visibility_freezes_before_later_definition() {
    let view = source("fn cold(){assert!({later!();true});macro_rules! later{()=>{true}}}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    assert!(work.sealed_declaration_expansions().is_err());
}

#[test]
fn cold_intrinsic_format_tail_discovers_impl_without_formatting_or_execution() {
    let (view, inputs) =
        source_inputs(r#"struct Global;fn cold(){assert!(true,"{}", {impl Global{}missing()});}"#);
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}

#[test]
fn cold_generated_intrinsic_discovers_nested_impl_argument_without_execution() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! make{()=>{{assert!({impl Global{}missing();true});true}}}fn cold(){make!();}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer macro demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let (_, changed) = source_inputs(
        "struct Global;macro_rules! make{()=>{{assert!({impl Global{}missing();false});true}}}fn cold(){make!();}",
    );
    changed.publish(&mut db).unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn cold_generated_intrinsic_inside_original_argument_preserves_records_and_budget() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! make{()=>{{assert!({impl Global{}missing();true});true}}}fn cold(){assert!(make!());}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("argument root absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}
#[test]
fn cold_generated_intrinsic_nested_unknown_argument_remains_incomplete() {
    let view =
        source("macro_rules! make{()=>{{assert!({unknown!();true});true}}}fn cold(){make!();}");
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_declaration_expansions().is_err());
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
}
#[test]
fn cold_generated_user_assert_expands_user_tokens_before_intrinsic_selection() {
    let view = source(
        "struct Global;macro_rules! assert{($value:expr)=>{{impl Global{}true}}}macro_rules! make{()=>{{assert!(missing());true}}}fn cold(){make!();}",
    );
    let mut work = discover_source_derives(&view);
    for count in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("user macro demand absent")
        };
        assert_eq!(work.accounted_expansion_count().unwrap(), count);
        work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
}

#[test]
fn cold_generated_qualified_intrinsic_uses_explicit_alias_under_disabled_preludes() {
    let (view, inputs) = source_inputs(
        "#![no_implicit_prelude]extern crate core as chosen;struct Global;macro_rules! make{()=>{{chosen::assert!({impl Global{}true});true}}}fn cold(){make!();}",
    );
    let mut work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest([242; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([243; 32]),
        Digest([244; 32]),
        std::sync::Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &ProfileIdentity::current(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}
#[test]
fn cold_generated_qualified_user_macro_does_not_acquire_compiler_identity() {
    let view = source(
        "macro_rules! make{()=>{{unknown::assert!({impl Global{}true});true}}}fn cold(){make!();}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    assert!(work.sealed_declaration_expansions().is_err());
}

#[test]
fn cold_generated_intrinsic_invalid_arity_refuses_before_scope_completion() {
    let view = source("macro_rules! make{()=>{{assert!();true}}}fn cold(){make!();}");
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert_eq!(
        work.poll().err().unwrap().code,
        "LRR-E-RUNTIME-MACRO-ARGUMENTS"
    );
    assert!(work.sealed_declaration_expansions().is_err());
}

#[test]
fn cold_generated_intrinsic_known_argument_macro_expands_actual_child() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! leaf{()=>{{impl Global{}true}}}macro_rules! make{()=>{{assert!(leaf!());true}}}fn cold(){make!();}",
    );
    let mut work = discover_source_derives(&view);
    for count in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("generated argument child demand absent")
        };
        assert_eq!(work.accounted_expansion_count().unwrap(), count);
        work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}

#[test]
fn cold_generated_argument_local_definition_preserves_actual_origin() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! make{()=>{{assert!({macro_rules! local{()=>{{impl Global{}true}}}local!()});true}}}fn cold(){make!();}",
    );
    let mut work = discover_source_derives(&view);
    for count in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("local generated argument demand absent")
        };
        assert_eq!(work.accounted_expansion_count().unwrap(), count);
        work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}
#[test]
fn cold_generated_argument_child_foreign_reply_and_stale_source_refuse() {
    let text = "struct Global;macro_rules! leaf{()=>{{impl Global{}true}}}macro_rules! make{()=>{{assert!(leaf!());true}}}fn cold(){make!();}";
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    let root = std::sync::Arc::new(request.expand().unwrap());
    work.admit_expansion(root.clone()).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request: child } = work.poll().unwrap() else {
        panic!("child absent")
    };
    assert!(work.admit_expansion(root).is_err());
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    work.admit_expansion(std::sync::Arc::new(child.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let (_, changed) = source_inputs(&text.replace("impl Global{}true", "impl Global{}false"));
    changed.publish(&mut db).unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn cold_recursive_generated_intrinsic_argument_tree_preserves_origin_and_records() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! leaf{()=>{{impl Global{}true}}}macro_rules! middle{()=>{{assert!(leaf!());true}}}macro_rules! root{()=>{{assert!(middle!());true}}}fn cold(){root!();}",
    );
    let mut work = discover_source_derives(&view);
    for count in 0..3 {
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("recursive argument demand absent")
        };
        assert_eq!(work.accounted_expansion_count().unwrap(), count);
        work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.sealed_declaration_expansions().unwrap().len(), 3);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}

#[test]
fn cold_nested_compiler_inert_attribute_keeps_impl_declarations_visible() {
    let (view, inputs) = source_inputs(
        "struct Global;fn cold(){#[allow(dead_code)] impl Global{fn marker(){missing();}}}",
    );
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}

#[test]
fn cold_nested_unknown_and_qualified_attributes_remain_incomplete() {
    for attribute in [
        "unknown",
        "foreign::allow(dead_code)",
        "::allow(dead_code)",
        "derive(Clone)",
    ] {
        let text = format!("struct Global;fn cold(){{#[{attribute}] impl Global{{}}}}");
        let view = source(&text);
        let mut work = discover_source_derives(&view);
        assert!(
            matches!(work.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
            "{attribute}"
        );
        assert!(
            !work
                .impl_header_inventory()
                .unwrap()
                .pending_producers()
                .is_empty()
        );
        assert!(work.sealed_expansions().is_err());
        let mut db = lrr::db::memo::Database::default();
        assert!(work.publish(&mut db).is_err());
    }
}
#[test]
fn cold_nested_inert_attribute_source_change_invalidates_publication() {
    let text = "struct Global;fn cold(){#[allow(dead_code)] impl Global{}}";
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    source_inputs(&text.replace("allow(dead_code)", "unknown"))
        .1
        .publish(&mut db)
        .unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn rustc_nested_inert_field_attribute_discovery_preserves_unchanged_source() {
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/attributes/issue-115264-expr-field.rs"
    );
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert!(work.impl_header_inventory().unwrap().headers().is_empty());
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    println!(
        "{}",
        serde_json::json!({"upstream_case":"tests/ui/attributes/issue-115264-expr-field.rs", "scope":"cold declaration discovery and certificate publication only", "runtime_body_prepared":false, "full_fixture_passed":false})
    );
}

#[test]
fn cold_intrinsic_argument_inert_attribute_preserves_impl_and_publication() {
    for text in [
        "struct Global;fn cold(){assert!({#[allow(dead_code)] impl Global{}true});}",
        "struct Global;macro_rules! make{()=>{{assert!({#[allow(dead_code)] impl Global{}true});true}}}fn cold(){make!();}",
    ] {
        let (view, inputs) = source_inputs(text);
        let mut work = discover_source_derives(&view);
        let mut records = 0;
        loop {
            match work.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => {
                    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                        .unwrap();
                    records += 1;
                }
                SourceDiscoveryStep::Sealed(_) => break,
                _ => panic!("inert argument producer remained unresolved: {text}"),
            }
        }
        assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
        assert_eq!(work.accounted_expansion_count().unwrap(), records);
        assert_eq!(records, usize::from(text.contains("macro_rules!")));
        let mut db = lrr::db::memo::Database::default();
        inputs.publish(&mut db).unwrap();
        work.publish(&mut db).unwrap();
    }
}

#[test]
fn cold_generated_inert_attribute_preserves_actual_record_and_impl() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! make{()=>{{#[allow(dead_code)] impl Global{}true}}}fn cold(){make!();}",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("root absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}

#[test]
fn cold_argument_and_generated_unknown_attributes_refuse_seals() {
    for attribute in [
        "unknown",
        "foreign::allow(dead_code)",
        "::allow(dead_code)",
        "derive(Clone)",
    ] {
        for text in [
            format!("struct Global;fn cold(){{assert!({{#[{attribute}] impl Global{{}}true}});}}"),
            format!(
                "struct Global;macro_rules! make{{()=>{{{{#[{attribute}] impl Global{{}}true}}}}}}fn cold(){{make!();}}"
            ),
            format!(
                "struct Global;macro_rules! make{{()=>{{{{assert!({{#[{attribute}] impl Global{{}}true}});true}}}}}}fn cold(){{make!();}}"
            ),
        ] {
            let view = source(&text);
            let mut work = discover_source_derives(&view);
            if text.contains("macro_rules!") {
                let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
                    panic!("root absent")
                };
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            assert!(
                matches!(work.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
                "{text}"
            );
            assert!(work.sealed_expansions().is_err());
            assert!(
                work.publish(&mut lrr::db::memo::Database::default())
                    .is_err()
            );
        }
    }
}
#[test]
fn cold_generated_inert_attribute_changed_record_source_refuses_publication() {
    let text = "struct Global;macro_rules! make{()=>{{#[allow(dead_code)] impl Global{}true}}}fn cold(){make!();}";
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("root absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    source_inputs(&text.replace("allow(dead_code)", "unknown"))
        .1
        .publish(&mut db)
        .unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}

#[test]
fn cold_nested_impl_macro_stays_behind_unknown_parent_attribute() {
    for text in [
        "struct Global;macro_rules! make{()=>{impl Global{}}}#[unknown] fn cold(){make!();}",
        "struct Global;macro_rules! make{()=>{impl Global{}}}fn cold(){#[unknown] fn nested(){make!();}}",
        "struct Global;macro_rules! make{()=>{impl Global{}}}fn cold(){#[unknown] {make!();}}",
    ] {
        let view = source(text);
        let mut work = discover_source_derives(&view);
        assert!(
            matches!(work.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
            "cold descendant expanded before unknown parent: {text}"
        );
        assert!(work.sealed_expansions().is_err());
        assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    }
}

#[test]
fn cold_multiple_inert_attributes_and_disabled_unknown_preserve_cfg_inventory() {
    let (view, inputs) = source_inputs(
        "struct Global;fn cold(){#[allow(dead_code)] #[doc=\"cold\"] #[deprecated] impl Global{}#[cfg(any())] #[unknown] impl Global{fn disabled(){missing();}}}",
    );
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
}

#[test]
fn cold_procedural_source_input_preserves_actual_tokens_slot_and_refuses_foreign_scan() {
    let text = "fn cold(){tool::identity!({impl Global{}true});missing();}";
    let view = source(text);
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let parsed = lrr::syntax::parse::ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let scan = parsed
        .scan_declarations(
            view.selection().path.clone(),
            view.outline().root_scope.clone(),
            &CfgSet::guest_default(),
            Default::default(),
        )
        .unwrap();
    let producer = scan
        .producers()
        .iter()
        .find(|p| p.kind == lrr::syntax::declaration_scan::DeclarationProducerKind::Macro)
        .unwrap();
    let input =
        lrr::service::function_macro_requests::FunctionInvocationInput::from_declaration_source(
            &view,
            &scan,
            producer.identity,
        )
        .unwrap();
    assert_eq!(input.grammar(), producer.grammar);
    assert_eq!(input.scope(), &producer.scope);
    assert!(!input.arguments().roots().is_empty());
    let changed = source(&text.replace("true", "false"));
    assert!(
        lrr::service::function_macro_requests::FunctionInvocationInput::from_declaration_source(
            &changed,
            &scan,
            producer.identity
        )
        .is_err()
    );
    assert!(
        lrr::service::function_macro_requests::FunctionInvocationInput::from_declaration_source(
            &view,
            &scan,
            Digest([255; 32])
        )
        .is_err()
    );
}

#[test]
fn cold_original_intrinsic_argument_unknown_producer_blocks_completion() {
    let v = source("struct Global;fn cold(){assert!({unknown!({impl Global{}true})});missing();}");
    let mut work = discover_source_derives(&v);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unknown argument producer falsely settled")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(blocked.iter().any(|(_, inputs)| !inputs.is_empty()));
    assert!(work.sealed_expansions().is_err());
    assert!(
        work.publish(&mut lrr::db::memo::Database::default())
            .is_err()
    );
    assert!(work.impl_header_inventory().unwrap().headers().is_empty());
}

#[test]
fn item_generated_cold_function_macro_impl_is_discovered_before_sealing() {
    let (view, inputs) = source_inputs(
        "struct Global;macro_rules! inner{()=>{{impl Global{fn marker(){missing();}}true}}}macro_rules! outer{()=>{fn cold(){inner!();let wrong:i32=\"cold\";missing();}}}outer!();",
    );
    let mut work = discover_source_derives(&view);
    let mut records = Vec::new();
    for _ in 0..4 {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                records.push(request.key().clone());
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => break,
            _ => panic!("generated cold function remained incomplete"),
        }
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(
        records.len(),
        2,
        "generated cold macro producer was omitted"
    );
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
    assert_eq!(work.sealed_expansions().unwrap().len(), 1);
    assert_eq!(work.sealed_declaration_expansions().unwrap().len(), 1);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(
            &lrr::db::provider::SyntaxProvider,
            lrr::db::key::QueryKey::DiscoverySeal(work.key().clone())
        ),
        lrr::db::poll::QueryPoll::Ready(Ok(_))
    ));
}

#[test]
fn item_generated_cold_unknown_producer_keeps_discovery_incomplete() {
    let view = source(
        "struct Global;macro_rules! outer{()=>{fn cold(){unknown!({impl Global{}});missing();}}}outer!();",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer demand absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unknown cold producer falsely settled")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
    assert!(
        work.publish(&mut lrr::db::memo::Database::default())
            .is_err()
    );
}
#[test]
fn item_generated_direct_cold_impl_is_indexed_without_phantom_expansion() {
    let view = source(
        "struct Global;macro_rules! outer{()=>{fn cold(){impl Global{fn marker(){missing();}}let wrong:i32=\"cold\";missing();}}}outer!();",
    );
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer demand absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 1);
    assert_eq!(work.accounted_expansion_count().unwrap(), 1);
    assert!(work.sealed_declaration_expansions().unwrap().is_empty());
}

#[test]
fn ordinary_generated_item_macro_is_not_reexecuted_for_declaration_discovery() {
    let view = source(
        "macro_rules! leaf{()=>{pub fn made(){missing();}}}macro_rules! seed{()=>{leaf!();}}seed!();",
    );
    let mut work = discover_source_derives(&view);
    let mut demands = Vec::new();
    for _ in 0..8 {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                demands.push(request.key().clone());
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                demands.push(request.key().clone());
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => break,
            _ => panic!("ordinary/generated shared producer incomplete"),
        }
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(
        demands.len(),
        2,
        "ordinary generated item producer was reexpanded by the declaration consumer"
    );
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
}

#[test]
fn scope_declaration_inventory_query_requires_publication_and_invalidates_after_source_edit() {
    use lrr::db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue};
    let text = "struct Global;macro_rules! inner{()=>{{impl Global{fn marker(){missing();}}true}}}macro_rules! outer{()=>{fn cold(){inner!();let wrong:i32=\"cold\";missing();}}}outer!();";
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    let query = QueryKey::ScopeDeclarationInventory(work.key().clone());
    assert!(
        matches!(db.fetch(&SyntaxProvider, query.clone()), QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { key }) if key == *work.key())
    );
    let mut expansions = 0;
    for _ in 0..4 {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
                expansions += 1;
            }
            SourceDiscoveryStep::Sealed(_) => break,
            _ => panic!("declaration inventory did not settle"),
        }
    }
    assert_eq!(expansions, 2);
    work.publish(&mut db).unwrap();
    let before = db.source_revision();
    let QueryPoll::Ready(Ok(first)) = db.fetch(&SyntaxProvider, query.clone()) else {
        panic!("published inventory absent")
    };
    let QueryValue::ScopeDeclarationInventory { key, inventory } = first.value.as_ref() else {
        panic!("inventory variant differs")
    };
    assert_eq!(key, work.key());
    assert_eq!(inventory.headers().len(), 1);
    assert_eq!(
        inventory.fingerprint(),
        work.impl_header_inventory().unwrap().fingerprint()
    );
    let QueryPoll::Ready(Ok(warm)) = db.fetch(&SyntaxProvider, query.clone()) else {
        panic!("warm inventory absent")
    };
    assert_eq!(first.fingerprint, warm.fingerprint);
    assert_eq!(
        db.source_revision(),
        before,
        "query mutated captured source"
    );
    assert!(
        !db.trace
            .counts
            .keys()
            .any(|key| key.starts_with("BodyStructure:") || key.starts_with("Thunk:")),
        "declaration query prepared executable source"
    );
    let (_, updated) = source_inputs(&text.replace("impl Global", "impl Another"));
    updated.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, query),
        QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { .. })
    ));
}

#[test]
fn scope_inventory_query_cannot_promote_pending_producers_or_issue_a_global_impl_seal() {
    use lrr::db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, seals::SealDomain};
    let (view, inputs) = source_inputs("fn cold(){unknown!({impl Global{}});missing();}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
    assert!(matches!(
        db.fetch(
            &SyntaxProvider,
            QueryKey::ScopeDeclarationInventory(work.key().clone())
        ),
        QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { .. })
    ));
    let mut global = work.key().clone();
    global.domain = SealDomain::ImplCatalog {
        id: Digest([255; 32]),
    };
    let QueryPoll::Ready(Err(error)) =
        db.fetch(&SyntaxProvider, QueryKey::ScopeDeclarationInventory(global))
    else {
        panic!("scope inventory was promoted to global certificate")
    };
    assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
}

#[test]
fn rustc_unresolved_todo_keeps_upstream_impl_inventory_query_pending() {
    use lrr::db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider};
    // Unchanged upstream source, examined in LRR's current component profile.
    // Its original edition/directives remain preserved. The unsupported todo!
    // frontier must not be guessed inert; this is not a full fixture pass.
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/lint/non-local-defs/weird-exprs.rs"
    );
    let (view, inputs) = source_inputs(text);
    let mut work = discover_source_derives(&view);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unresolved upstream producer was falsely completed")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert_eq!(work.impl_header_inventory().unwrap().headers().len(), 6);
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
    assert!(
        matches!(db.fetch(&SyntaxProvider, QueryKey::ScopeDeclarationInventory(work.key().clone())), QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { key }) if key == *work.key())
    );
    assert!(
        !db.trace
            .counts
            .keys()
            .any(|key| key.starts_with("BodyStructure:") || key.starts_with("Thunk:")),
        "upstream declaration query prepared executable source"
    );
}

#[test]
fn scope_inventory_retained_handle_remains_accounted_after_certificate_replacement() {
    use lrr::db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue};
    let (view, inputs) =
        source_inputs("struct Global;fn cold(){impl Global{fn old(){missing();}}}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = QueryKey::ScopeDeclarationInventory(work.key().clone());
    let QueryPoll::Ready(Ok(result)) = db.fetch(&SyntaxProvider, key.clone()) else {
        panic!("original inventory absent")
    };
    let QueryValue::ScopeDeclarationInventory { inventory, .. } = result.value.as_ref() else {
        panic!("wrong inventory variant")
    };
    let held = inventory.clone();
    drop(result);
    let (new_view, new_inputs) =
        source_inputs("struct Global;fn cold(){impl Global{fn new(){missing();}}}");
    let mut new_work = discover_source_derives(&new_view);
    assert!(matches!(
        new_work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    new_inputs.publish(&mut db).unwrap();
    new_work.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(new_result)) = db.fetch(&SyntaxProvider, key) else {
        panic!("replacement inventory absent")
    };
    drop(new_result);
    let retained = db.cache_usage().unwrap();
    assert_eq!(held.headers().len(), 1);
    drop(held);
    let released = db.cache_usage().unwrap();
    assert!(
        retained.pinned_memo_bytes > released.pinned_memo_bytes,
        "retained inventory allocation escaped accounting"
    );
    assert!(retained.memo_bytes > released.memo_bytes);
}

#[test]
fn scope_inventory_zero_epoch_is_invalid_before_requesting_discovery() {
    use lrr::db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider};
    let view = source("struct Global;fn cold(){impl Global{}}");
    let work = discover_source_derives(&view);
    let mut invalid = work.key().clone();
    invalid.epoch = 0;
    let query = QueryKey::ScopeDeclarationInventory(invalid);
    let mut db = lrr::db::memo::Database::default();
    let QueryPoll::Ready(Err(error)) = db.fetch(&SyntaxProvider, query.clone()) else {
        panic!("invalid epoch became an effectful discovery request")
    };
    assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
    assert!(
        matches!(db.memo(&query).unwrap().value.as_ref(), lrr::db::value::QueryValue::Error(error) if error.code == "LRR-E-QUERY-REQUEST")
    );
    assert!(
        matches!(db.fetch(&SyntaxProvider, query), QueryPoll::Ready(Err(error)) if error.code == "LRR-E-QUERY-REQUEST")
    );
}

#[test]
fn scope_inventory_epoch_transition_refuses_old_certificate_and_requires_new_publication() {
    use lrr::db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue};
    let (view, inputs) =
        source_inputs("struct Global;fn cold(){impl Global{fn marker(){missing();}}}");
    let mut work = discover_source_derives(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    work.publish(&mut db).unwrap();
    let first_key = QueryKey::ScopeDeclarationInventory(work.key().clone());
    let QueryPoll::Ready(Ok(first)) = db.fetch(&SyntaxProvider, first_key.clone()) else {
        panic!("first epoch inventory absent")
    };
    let QueryValue::ScopeDeclarationInventory { inventory, .. } = first.value.as_ref() else {
        panic!("first inventory variant differs")
    };
    let inventory_fingerprint = inventory.fingerprint();
    let source_revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    let QueryPoll::Ready(Err(error)) = db.fetch(&SyntaxProvider, first_key) else {
        panic!("previous epoch returned a completed inventory")
    };
    assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
    let mut next_work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest([242; 32]),
        2,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap();
    let next_key = QueryKey::ScopeDeclarationInventory(next_work.key().clone());
    assert!(
        matches!(db.fetch(&SyntaxProvider, next_key.clone()), QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { key }) if key == *next_work.key())
    );
    assert!(db.memo(&next_key).is_none());
    assert!(matches!(
        next_work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    next_work.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(second)) = db.fetch(&SyntaxProvider, next_key) else {
        panic!("new epoch inventory absent after actual publication")
    };
    let QueryValue::ScopeDeclarationInventory { inventory, .. } = second.value.as_ref() else {
        panic!("new inventory variant differs")
    };
    assert_eq!(inventory.fingerprint(), inventory_fingerprint);
    assert_ne!(second.fingerprint, first.fingerprint);
    assert_eq!(db.source_revision(), source_revision);
}

#[test]
fn compiler_only_test_startup_checks_inert_aliases_on_both_catalog_paths() {
    for name in ["ignore", "should_panic"] {
        for late in [false, true] {
            let (view, inputs) = source_inputs(&format!(
                "use core::prelude::rust_2024::test as {name}; #[test] #[{name}] fn selected() {{}}"
            ));
            let compiler = std::sync::Arc::new(
                lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &ProfileIdentity::current(),
                )
                .unwrap(),
            );
            let discovery = SourceDiscovery::for_tests(
                vec![view.share()],
                view.outline().root_scope.clone(),
                Digest([242; 32]),
                1,
            )
            .unwrap();
            let mut discovery = if late {
                discovery
                    .with_declarative(Digest([243; 32]), Digest([244; 32]))
                    .unwrap()
                    .with_compiler(compiler)
                    .unwrap()
            } else {
                discovery
                    .with_declarative_compiler(Digest([243; 32]), Digest([244; 32]), compiler)
                    .unwrap()
            };
            let mut db = lrr::db::memo::Database::default();
            db.trace.detailed = true;
            inputs.publish(&mut db).unwrap();
            let error = match discovery.poll() {
                Err(error) => error,
                Ok(_) => panic!(
                    "compiler-only alias {name}, late={late} bypassed Attribute-kind discovery"
                ),
            };
            assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
            assert!(discovery.publish(&mut db).is_err());
            assert!(discovery.test_catalog(&mut db).is_err());
            assert_eq!(discovery.accounted_expansion_count().unwrap(), 0);
            assert!(db.trace.events.iter().all(|event| !matches!(
                event.key,
                lrr::db::key::QueryKey::FunctionSignature(_)
                    | lrr::db::key::QueryKey::BodyStructure(_)
                    | lrr::db::key::QueryKey::Thunk(_)
            )));
        }
    }
}
