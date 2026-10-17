//! Genuine source-bound scalar recipes only; no full stock-body pass here.
//! Attach this module below vm/frame.rs so only tests can inspect frame slots.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        hygiene::HygieneFacts,
        names::NameNamespaces,
        primitive_constants::{
            FloatingConstantOutcome, FloatingConstantRecipe, FloatingConstantSite,
            plan_floating_constant,
        },
        source_locals::PatternResolution,
        source_macros::PublishedMacroIndex,
    },
    ir::{
        op::{Op, PrimitiveType},
        pc::Phase,
    },
    runtime::value::ValueRepr,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParseKey, spans::BodySpan, token_wire as runtime},
    traits::term::TyTerm,
    vm::step::{StepOutcome, step},
};
use lrr_expand_adapter::{
    hygiene::HygieneStore,
    token_wire::{Edition, Key, TokenForest},
};
pub(super) const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float-nan.rs"
);

pub(super) fn world(
    source: &str,
    function: &str,
    edition: u16,
) -> (
    Database,
    PreparedSourceView,
    PublishedMacroIndex,
    BodyKey,
    NameNamespaces,
    Vec<crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
) {
    let unit = Digest::of_bytes(b"LRR original floating constant source");
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("scalar-constants.rs").unwrap();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
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
        panic!("source missing")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut hygiene = HygieneStore::default();
    let context = hygiene
        .root(
            Key(unit.0),
            match edition {
                2015 => Edition::E2015,
                2018 => Edition::E2018,
                2021 => Edition::E2021,
                2024 => Edition::E2024,
                _ => unreachable!(),
            },
        )
        .unwrap();
    let keys = [context];
    let rows = hygiene.resolution_contexts(&keys).unwrap();
    let names = hygiene.name_contexts(&keys).unwrap();
    let wire = TokenForest::new(vec![], hygiene.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let decoded = runtime::TokenForest::decode(&wire).unwrap();
    let facts = HygieneFacts::new(
        decoded.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let published =
        PublishedMacroIndex::from_view(view.share(), runtime::Key(context.0), facts).unwrap();
    let mut proofs = Vec::new();
    let published = if source == ORIGINAL {
        use crate::expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep};
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Type,
            Digest::of_bytes(b"floating constant completed discovery"),
            1,
        )
        .unwrap()
        .with_declarative(
            Digest::of_bytes(b"floating constant source expansions"),
            Digest::of_bytes(b"floating constant source generations"),
        )
        .unwrap()
        .with_compiler(std::sync::Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &view.selection().outline.parse.profile,
            )
            .unwrap(),
        ))
        .unwrap();
        // The library's own `assert!` definitions expand during discovery.
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                    panic!("original discovery incomplete: {diagnostic:?}")
                }
                _ => panic!("original discovery requires unestablished macro authority"),
            }
        }
        discovery.publish(&mut db).unwrap();
        let input = db
            .input(&crate::db::dependency::InputKey::DiscoverySeal(
                discovery.key().clone(),
            ))
            .unwrap();
        let crate::db::dependency::InputValue::DiscoverySeal(snapshot) = input.as_ref() else {
            panic!("sealed discovery input absent")
        };
        proofs.extend(
            snapshot
                .runtime_no_item_proofs
                .iter()
                .map(|(_, proof)| proof.clone()),
        );
        let mut indexes = discovery.completed_runtime_macro_indexes().unwrap();
        assert_eq!(indexes.len(), 1);
        let (completed_view, index) = indexes.pop().unwrap();
        assert_eq!(completed_view.selection(), view.selection());
        index
    } else {
        published
    };
    (db, view, published, body, graph, proofs)
}

pub(super) fn span(view: &PreparedSourceView, body: &BodyKey, at: usize, path: &str) -> BodySpan {
    let range = view
        .outline()
        .items
        .iter()
        .filter_map(|item| item.body.as_ref())
        .find(|handle| handle.key() == *body)
        .unwrap()
        .range;
    BodySpan {
        body_key: body.clone(),
        relative_start: u32::try_from(at).unwrap() - range.start,
        relative_end: u32::try_from(at + path.len()).unwrap() - range.start,
    }
}

struct Context {
    frame: Frame,
    store: GuestStore,
    lowered: FloatingConstantRecipe,
    expected: ValueRepr,
    primitive: PrimitiveType,
    destination: SlotId,
}
fn context(source: &str, qualifier: &str, member: &str, edition: u16) -> Context {
    let function = if source == ORIGINAL { "main" } else { "live" };
    let (mut db, view, published, body, graph, proofs) = world(source, function, edition);
    let mut entered = published.enter_body(view.selection(), &body).unwrap();
    let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
        &view.selection().outline.parse.profile,
    )
    .unwrap();
    for proof in proofs.iter().filter(|proof| proof.body() == &body) {
        entered
            .accept_no_item_proof(view.selection(), &body, proof, &catalog)
            .unwrap();
    }
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|binding| (binding.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(view.selection(), &body, &resolutions, &[])
        .unwrap();
    let path = format!("{qualifier}::{member}");
    let at = source.find(&path).unwrap();
    let written = span(&view, &body, at, &path);
    let site = FloatingConstantSite {
        source: &view,
        body: &body,
        at: at as u64,
        span: &written,
        qualifier,
        member,
        context: locals.context(),
        expected: None,
    };
    let FloatingConstantOutcome::Ready(plan) =
        plan_floating_constant(&mut db, &graph, &locals, site)
            .into_ready()
            .unwrap()
            .unwrap()
    else {
        panic!("source planner did not admit exact path")
    };
    let indexed = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!("wrong index variant")
    };
    let index = index.as_ref().clone();
    let node = index
        .nodes()
        .iter()
        .find(|node| index.span(node.id).unwrap() == written)
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let lowered = plan
        .lower_recipe(&mut db, &graph, &locals, site, &index, &pc)
        .into_ready()
        .unwrap()
        .unwrap();
    lowered
        .revalidate(&mut db, &graph, &locals, site, &index, &pc)
        .into_ready()
        .unwrap()
        .unwrap();
    let Op::PrimitiveFloatingConstant {
        constant,
        destination,
    } = lowered.recipe().recipe().operations.last().unwrap()
    else {
        panic!("closed proof operation missing")
    };
    let (expected, primitive, destination) =
        (constant.scalar(), constant.primitive(), *destination);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(index.clone(), pc, &mut store).unwrap();
    // Enter only ancestors outside the recipe's exact charged scope prefix.
    let first = lowered.recipe().recipe().operations.first().unwrap();
    let mut scope = match first {
        Op::ScopeEnter { scope } => index.scopes()[scope.0 as usize].parent,
        _ => Some(node.scope),
    };
    let mut ancestors = vec![];
    while let Some(id) = scope {
        ancestors.push(id);
        scope = index.scopes()[id.0 as usize].parent;
    }
    for id in ancestors.into_iter().rev() {
        frame.enter(id).unwrap();
    }
    Context {
        frame,
        store,
        lowered,
        expected,
        primitive,
        destination,
    }
}
fn run(c: &mut Context) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, c.lowered.recipe())
}
fn assert_written(c: &Context) {
    let cell = c.frame.cell(c.destination).unwrap();
    assert_eq!(c.store.read_scalar(cell).unwrap(), c.expected);
    assert_eq!(
        c.frame.source_view(c.destination).unwrap(),
        Some(TyTerm::Primitive(c.primitive))
    );
    assert_eq!(c.frame.counters.started_by_opcode[41], 1);
}
#[test]
fn closed_ieee_runtime_constants_keep_both_widths_and_nonfinite_bit_patterns() {
    for qualifier in ["f32", "f64"] {
        for member in [
            "NAN",
            "INFINITY",
            "NEG_INFINITY",
            "MIN",
            "MAX",
            "MIN_POSITIVE",
            "EPSILON",
        ] {
            let source = format!("fn live()->{qualifier}{{{qualifier}::{member}}}");
            let mut c = context(&source, qualifier, member, 2024);
            assert!(matches!(run(&mut c).unwrap(), StepOutcome::Next(_)));
            assert_written(&c);
            c.expected.validate_scalar().unwrap();
            let value = if qualifier == "f32" {
                assert_eq!((c.expected.kind, c.expected.type_id), (6, 14));
                assert!(c.expected.payload <= u64::from(u32::MAX));
                f64::from(f32::from_bits(c.expected.payload as u32))
            } else {
                assert_eq!((c.expected.kind, c.expected.type_id), (7, 15));
                f64::from_bits(c.expected.payload)
            };
            match member {
                "NAN" => assert!(value.is_nan()),
                "INFINITY" => assert_eq!(value, f64::INFINITY),
                "NEG_INFINITY" => assert_eq!(value, f64::NEG_INFINITY),
                _ => assert!(value.is_finite()),
            }
        }
    }
}
