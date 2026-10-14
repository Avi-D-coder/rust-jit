use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    expand::{
        scopes::MacroDefinition,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    facade::macros::RuntimeMacroKind,
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        macro_names::PathLookup,
        runtime_macro_catalog::RuntimeMacroCatalog,
        runtime_macro_plan::{RuntimeMacroPreparation, prepare_runtime_macro},
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        parse::{ParseKey, ParsedFile},
        runtime_macro_arguments::VecForm,
        spans::SourceRange,
    },
};
fn profile(edition: u16) -> ProfileIdentity {
    let mut p = ProfileIdentity::current();
    p.edition = edition;
    p
}
fn at(text: &str, part: &str) -> SourceRange {
    let start = u32::try_from(text.find(part).unwrap()).unwrap();
    SourceRange {
        start,
        end: start + u32::try_from(part.len()).unwrap(),
    }
}
fn parse_source(text: &str, edition: u16) -> ParsedFile {
    ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        profile(edition),
    )
    .unwrap()
}
fn source(text: &str, edition: u16) -> (PreparedSourceView, PreparedSourceInputs) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = profile(edition);
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let crate_key = Digest([211; 32]);
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
            blob,
            profile,
            cfg,
            path,
            crate_key,
            module_path: vec![],
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source not selected")
    };
    (view, inputs)
}
fn discovery(view: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        Digest([212; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([213; 32]),
        Digest([214; 32]),
        std::sync::Arc::new(
            CompileInputCatalog::standalone(&view.selection().outline.parse.profile).unwrap(),
        ),
    )
    .unwrap()
}
fn definition(catalog: &RuntimeMacroCatalog, kind: RuntimeMacroKind) -> MacroDefinition {
    catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap() == Some(kind))
        .unwrap()
        .clone()
}
#[test]
fn vec_original_cold_sibling_declaration_facet_settles_four_editions() {
    let text =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/expr-match-panic.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (view, _) = source(text, edition);
        let mut work = discovery(&view);
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        assert_eq!(work.accounted_expansion_count().unwrap(), 0);
        assert!(work.impl_header_inventory().unwrap().headers().is_empty());
        // Whole main/test_box unexecuted here; root separately exercises selected test_simple.
    }
}
#[test]
fn vec_alloc_definition_std_reexport_preserves_world_and_user_identity() {
    for edition in [2015, 2018, 2021, 2024] {
        let catalog = RuntimeMacroCatalog::standard(&profile(edition)).unwrap();
        let vec = definition(&catalog, RuntimeMacroKind::Vec);
        let core = definition(&catalog, RuntimeMacroKind::Panic).defining_unit;
        let std = definition(&catalog, RuntimeMacroKind::Println).defining_unit;
        assert_ne!(vec.defining_unit, core);
        assert_ne!(vec.defining_unit, std);
        assert_eq!(vec.definition_edition, edition);
        assert_eq!(
            RuntimeMacroCatalog::new(core, edition)
                .unwrap()
                .select(&vec)
                .unwrap(),
            None
        );
        let mut forged = vec.clone();
        forged.defining_unit = std;
        assert_eq!(
            catalog.select(&forged).unwrap_err().code,
            "LRR-E-RUNTIME-MACRO"
        );
        let user = MacroDefinition {
            identity: Digest([215; 32]),
            defining_unit: std,
            definition_edition: edition,
        };
        assert_eq!(catalog.select(&user).unwrap(), None);
        assert_eq!(
            RuntimeMacroCatalog::new(core, edition)
                .unwrap()
                .definitions()
                .count(),
            4
        );
    }
}
#[test]
fn vec_empty_list_repeat_arguments_keep_exact_spans_and_no_item_proof() {
    for edition in [2015, 2018, 2021, 2024] {
        let catalog = RuntimeMacroCatalog::standard(&profile(edition)).unwrap();
        let def = definition(&catalog, RuntimeMacroKind::Vec);
        for (call, form, parts) in [
            ("vec![]", VecForm::Empty, vec![]),
            ("vec![one(), 2,]", VecForm::List, vec!["one()", "2"]),
            (
                "vec![one(); count()]",
                VecForm::Repeat,
                vec!["one()", "count()"],
            ),
        ] {
            let text = format!("fn live(){{let _={call};}}");
            let parsed = parse_source(&text, edition);
            let invocation = parsed.macro_call_at(at(&text, call)).unwrap();
            let RuntimeMacroPreparation::Intrinsic(plan) = prepare_runtime_macro(
                &parsed,
                invocation,
                PathLookup::Defined(def.clone()),
                &catalog,
            )
            .unwrap() else {
                panic!("intrinsic absent")
            };
            assert_eq!(plan.arguments().vec_form(), Some(form));
            assert_eq!(
                plan.arguments().operands(),
                parts.iter().map(|p| at(&text, p)).collect::<Vec<_>>()
            );
            assert_eq!(plan.arguments().message_argument_count(), 0);
            assert!(plan.arguments().message().is_none());
            let outline = parsed
                .outline(Digest([211; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            assert!(
                plan.no_item_proof(&outline.items[0].body.as_ref().unwrap().key())
                    .is_some()
            );
            let other = parse_source("fn live(){vec![3]}", edition);
            assert!(plan.arguments().require_source(&other.key()).is_err());
        }
        let text = "fn live(){vec![1;2,]}";
        let parsed = parse_source(text, edition);
        let invocation = parsed.macro_call_at(at(text, "vec![1;2,]")).unwrap();
        assert_eq!(
            parsed
                .runtime_macro_arguments(&invocation, RuntimeMacroKind::Vec)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-MACRO-ARGUMENTS"
        );
    }
}
#[test]
fn vec_nested_declarations_macros_attributes_do_not_receive_no_item_certificate() {
    let catalog = RuntimeMacroCatalog::standard(&profile(2024)).unwrap();
    let def = definition(&catalog, RuntimeMacroKind::Vec);
    for call in [
        "vec![{struct Hidden;1}]",
        "vec![unknown!()]",
        "vec![#[cfg(any())] 1]",
        "vec![1;unknown!()]",
    ] {
        let text = format!("fn live(){{let _={call};}}");
        let parsed = parse_source(&text, 2024);
        let invocation = parsed.macro_call_at(at(&text, call)).unwrap();
        let RuntimeMacroPreparation::Intrinsic(plan) = prepare_runtime_macro(
            &parsed,
            invocation,
            PathLookup::Defined(def.clone()),
            &catalog,
        )
        .unwrap() else {
            panic!("intrinsic absent")
        };
        let outline = parsed
            .outline(Digest([211; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        assert!(
            plan.no_item_proof(&outline.items[0].body.as_ref().unwrap().key())
                .is_none()
        );
    }
    let (view, _) = source("fn cold(){vec![unknown!()];}fn main(){}", 2024);
    let mut work = discovery(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.accounted_expansion_count().unwrap(), 0);
}
#[test]
fn vec_shadowed_user_definition_preserves_normal_expansion_authority() {
    let (view, _) = source(
        "struct Global;macro_rules! vec{($e:expr)=>{{impl Global{}7}}}fn cold(){vec![1];}fn main(){}",
        2024,
    );
    let mut work = discovery(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("user vec treated as intrinsic")
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
fn vec_reached_allocation_refuses_before_operand_execution_or_guest_effect() {
    use lrr::{
        db::poll::QueryPoll,
        engine::{
            run::{RunMachine, RunPoll},
            standalone_macros::CompletedStandaloneRuntimeMacros,
        },
    };
    let (view, inputs) = source("fn live(){vec![unknown()];}fn main(){}", 2024);
    let mut work = discovery(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let macros = CompletedStandaloneRuntimeMacros::from_discovery(&view, &work).unwrap();
    let contexts = macros.contexts().unwrap();
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "live")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let revision = db.source_revision();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut machine)) =
        RunMachine::start_with_discovery(&mut db, body, 100_000, work)
    else {
        panic!("startup")
    };
    let RunPoll::Fault(error) = machine.poll_with_runtime_macros(&mut db, &contexts) else {
        panic!("reached vec must not succeed")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert!(error.message.contains("Vec allocation"));
    assert_eq!(db.source_revision(), revision);
    db.finish_run().unwrap();
}

#[test]
fn vec_prelude_paths_and_renames_do_not_add_an_alloc_extern_alias() {
    for text in [
        "#![no_std] fn cold(){vec![1];}",
        "fn cold(){alloc::vec![1];}",
    ] {
        let (view, _) = source(text, 2024);
        let mut work = discovery(&view);
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
    }
    for text in [
        "#![no_implicit_prelude] extern crate std; fn cold(){std::vec![1];}",
        "use std::vec as make;fn cold(){make![1];}",
    ] {
        let (view, _) = source(text, 2024);
        let mut work = discovery(&view);
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        assert_eq!(work.accounted_expansion_count().unwrap(), 0);
    }
}
