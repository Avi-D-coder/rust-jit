//! Read-only origin observation of actual unchanged stock expression expansions.
//! These component assertions do not execute or count a full stock fixture.
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
    .with_declarative(
        Digest([6; 32]),
        Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
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
        db::{memo::Database, poll::QueryPoll},
        ir::pc::Phase,
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
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    discovery.publish(&mut db).unwrap();
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
        let QueryPoll::Ready(Ok(plan)) =
            lrr::engine::runtime_macros::prepare_declarative_expression(
                &mut db,
                source,
                &body,
                &pc,
                &record.descriptor(),
            )
        else {
            panic!("exact published stock literal did not prepare")
        };
        let trace = db.trace.events.len();
        let usage = serde_json::to_value(db.cache_usage().unwrap()).unwrap();
        let origin =
            lrr::engine::fixture_observer::observe_declarative_literal(&db, &views, &body, &plan)
                .unwrap();
        assert_eq!(db.trace.events.len(), trace);
        assert_eq!(
            serde_json::to_value(db.cache_usage().unwrap()).unwrap(),
            usage
        );
        assert_eq!(origin.expansion, record.descriptor().fingerprint());
        assert_eq!(origin.invocation_identity, record.key().invocation);
        assert_eq!(origin.body, plan.recipe().recipe().origin.span);
        let caller = origin.invocation.unwrap();
        assert_eq!(caller.source.unit.0, source.selection().outline.crate_key.0);
        assert_eq!(
            caller.source.content.0,
            source.selection().outline.parse.content.0
        );
        assert_eq!(caller.range.start, handle.range.start + node.range.start);
        assert_eq!(caller.range.end, handle.range.start + node.range.end);
        let owner = origin.definition.unwrap();
        assert_eq!(owner.identity, definition);
        assert_eq!(owner.name, name);
        assert_eq!(
            owner.location.source.content.0,
            views[1].selection().outline.parse.content.0
        );
        assert!(!origin.tokens.is_empty());
        assert!(
            origin
                .tokens
                .iter()
                .all(|span| span.edition == lrr::syntax::token_wire::Edition::E2015)
        );
        assert!(
            origin
                .tokens
                .iter()
                .all(|span| span.source.unit.0 == views[1].selection().outline.crate_key.0)
        );
        assert!(origin.tokens.iter().all(|span| {
            plan.tokens().contexts().iter().any(|context| {
                context.key() == span.context
                    && context.edition == lrr::syntax::token_wire::Edition::E2015
            })
        }));
        let main = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let foreign_body = source
            .index_body(source.selection(), &main.key(), BodyIndexLimits::default())
            .unwrap();
        assert!(
            lrr::engine::fixture_observer::observe_declarative_literal(
                &db,
                &views,
                &foreign_body,
                &plan
            )
            .is_err(),
            "another caller body must refuse origin observation"
        );
        let (changed_views, _, _) = prepare(year, &format!("{text}\n// changed source capture\n"));
        assert!(
            lrr::engine::fixture_observer::observe_declarative_literal(
                &db,
                &changed_views,
                &body,
                &plan
            )
            .is_err(),
            "another source revision must refuse origin observation"
        );
        let mut absent = Database::default();
        inputs.publish(&mut absent).unwrap();
        assert!(
            lrr::engine::fixture_observer::observe_declarative_literal(
                &absent, &views, &body, &plan
            )
            .is_err(),
            "unpublished record must refuse origin observation"
        );
        assert!(
            lrr::engine::fixture_observer::observe_declarative_literal(
                &db,
                &[source.share()],
                &body,
                &plan
            )
            .is_err(),
            "absent captured definition owner must refuse origin observation"
        );
    }
}
#[test]
fn unchanged_2015_literal_expression_origin_observation() {
    exercise(2015, ROOT2015)
}
#[test]
fn unchanged_2018_literal_expression_origin_observation() {
    exercise(2018, ROOT2018)
}

// Harness adaptations keep the untouched 2018 bytes, selecting later owning
// profiles only; these are origin component assertions, not full stock passes.
#[test]
fn untouched_2018_bytes_as_2021_literal_origin_observation() {
    exercise(2021, ROOT2018)
}
#[test]
fn untouched_2018_bytes_as_2024_literal_origin_observation() {
    exercise(2024, ROOT2018)
}
