//! Preparation of sealed expansions from unchanged stock bodies, not full execution.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const ROOT2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2015-2015.rs"
);
const ROOT2018: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2018-2015.rs"
);
const AUX2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/auxiliary/edition-kw-macro-2015.rs"
);
fn prepare(
    year: u16,
    text: &str,
) -> (
    Vec<lrr::source::changes::PreparedSourceView>,
    SourceDiscovery,
    PreparedSourceInputs,
) {
    let mut root_profile = ProfileIdentity::current();
    root_profile.edition = year;
    let mut aux_profile = ProfileIdentity::current();
    aux_profile.edition = 2015;
    let units = vec![
        SourceUnitInput {
            path: SourcePath::new("caller.rs").unwrap(),
            crate_key: Digest([2; 32]),
            module_path: vec![],
            profile: root_profile,
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        },
        SourceUnitInput {
            path: SourcePath::new("edition-kw-macro-2015.rs").unwrap(),
            crate_key: Digest([1; 32]),
            module_path: vec![],
            profile: aux_profile,
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(AUX2015.as_bytes().to_vec()).unwrap(),
        },
    ];
    let selections = units
        .iter()
        .map(|unit| SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: unit.profile.clone(),
                },
                crate_key: unit.crate_key,
                module_path: vec![],
                cfg: unit.cfg.fingerprint(),
            },
        })
        .collect::<Vec<_>>();
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![SourceDependency {
            consumer: Digest([2; 32]),
            alias: "edition_kw_macro_2015".into(),
            provider: Digest([1; 32]),
        }],
    )
    .unwrap();
    let views = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("source absent")
            };
            view
        })
        .collect::<Vec<_>>();
    let discovery = SourceDiscovery::new(
        views.iter().map(|view| view.share()).collect(),
        views[0].outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([5; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([6; 32]),
        lrr::expand::source_declarative::engine_version(),
        std::sync::Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &views[0].selection().outline.parse.profile,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    (views, discovery, inputs)
}
fn sealed(
    year: u16,
    text: &str,
) -> (
    Vec<lrr::source::changes::PreparedSourceView>,
    SourceDiscovery,
    PreparedSourceInputs,
) {
    let (views, mut discovery, inputs) = prepare(year, text);
    for _ in 0..1000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => return (views, discovery, inputs),
            _ => panic!("unexpected stock discovery authority or incomplete result"),
        }
    }
    panic!("stock discovery did not seal")
}
fn exercise(year: u16, text: &str) {
    use lrr::{
        db::{
            dependency::InputKey,
            memo::Database,
            poll::{InputRequest, QueryPoll},
        },
        ir::{op::Op, pc::Phase},
        syntax::body_index::{BodyIndexLimits, ExpressionKind, NodeRole},
    };
    let (views, discovery, inputs) = sealed(year, text);
    let source = &views[0];
    let handle = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "check_async")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = source
        .index_body(
            source.selection(),
            &handle.key(),
            BodyIndexLimits::default(),
        )
        .unwrap();
    let records = discovery.sealed_declaration_expansions().unwrap();
    for name in ["consumes_async", "consumes_async_raw"] {
        let definition = views[1]
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap()
            .def_key
            .fingerprint();
        let record = records.iter().find(|record| matches!(record.key().macro_identity, lrr::expand::identity::MacroIdentity::Declaration { definition: id } if id == definition)).unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|node| {
                node.role == NodeRole::Expression(ExpressionKind::Macro)
                    && text[usize::try_from(handle.range.start + node.range.start).unwrap()..]
                        .starts_with(&format!("{name}!("))
            })
            .unwrap();
        let pc = body.pc(node.id, Phase::Entry).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        match lrr::engine::runtime_macros::prepare_declarative_expression(
            &mut db,
            source,
            &body,
            &pc,
            &record.descriptor(),
        ) {
            QueryPoll::NeedInput(InputRequest::Expand(request)) => {
                assert_eq!(request.key, *record.key())
            }
            other => panic!("missing record must suspend: {other:?}"),
        }
        discovery.publish(&mut db).unwrap();
        let plan = match lrr::engine::runtime_macros::prepare_declarative_expression(
            &mut db,
            source,
            &body,
            &pc,
            &record.descriptor(),
        ) {
            QueryPoll::Ready(Ok(plan)) => plan,
            other => panic!("sealed stock expansion must prepare: {other:?}"),
        };
        let recipe = plan.recipe().recipe();
        let ordinary = lrr::ir::recipe::RecipeDraft {
            key: recipe.key.clone(),
            origin: recipe.origin.clone(),
            operations: recipe.operations.to_vec(),
            exits: vec![recipe.exit.clone()],
            dependencies: recipe.dependencies.to_vec(),
        };
        assert!(
            lrr::ir::validate::verify(ordinary, &body).is_err(),
            "public verifier must still reject unbound ancestry"
        );

        assert!(recipe.operations.iter().any(|op| matches!(op, Op::Literal { literal, destination, .. } if literal.spelling == "1" && Some(*destination) == node.value)));
        assert_eq!(
            recipe.origin.expansion_ancestry,
            vec![record.descriptor().fingerprint()]
        );
        assert!(recipe.dependencies.iter().any(|stamp| matches!(stamp, lrr::db::dependency::DependencyStamp::Input { key: InputKey::ExpansionRecord(key), .. } if *key == record.key().fingerprint())));
        assert_eq!(
            plan.tokens().encode().unwrap(),
            record.tokens().unwrap().encode().unwrap()
        );
        assert!(!plan.origins().is_empty());
        assert!(
            plan.metadata_bytes()
                >= plan.recipe().metadata_bytes() + plan.tokens().allocated_bytes().unwrap()
        );
        let wrong = records
            .iter()
            .find(|other| {
                other.key().invocation != record.key().invocation
                    && other.key().grammar == record.key().grammar
            })
            .unwrap();
        assert!(matches!(
            lrr::engine::runtime_macros::prepare_declarative_expression(
                &mut db,
                source,
                &body,
                &pc,
                &wrong.descriptor()
            ),
            QueryPoll::Ready(Err(_))
        ));
        let mut changed_output = record.descriptor();
        changed_output.output = Digest([99; 32]);
        assert!(matches!(
            lrr::engine::runtime_macros::prepare_declarative_expression(
                &mut db,
                source,
                &body,
                &pc,
                &changed_output
            ),
            QueryPoll::Ready(Err(_))
        ));
        let exit = body.pc(node.id, Phase::Exit).unwrap();
        assert!(matches!(
            lrr::engine::runtime_macros::prepare_declarative_expression(
                &mut db,
                source,
                &body,
                &exit,
                &record.descriptor()
            ),
            QueryPoll::Ready(Err(_))
        ));
    }
}
#[test]
fn unchanged_2015_body_literal_macro_preparation() {
    exercise(2015, ROOT2015)
}
#[test]
fn unchanged_2018_body_literal_macro_preparation() {
    exercise(2018, ROOT2018)
}

// Harness edition adaptations retain the untouched 2018 fixture bytes while
// checking the later official owning profiles; these are component assertions.
#[test]
fn untouched_2018_bytes_as_2021_literal_preparation() {
    exercise(2021, ROOT2018)
}
#[test]
fn untouched_2018_bytes_as_2024_literal_preparation() {
    exercise(2024, ROOT2018)
}
