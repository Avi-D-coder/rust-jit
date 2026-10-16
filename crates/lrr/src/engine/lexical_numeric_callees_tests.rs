//! Genuine sealed-source lexical identity controls; no whole-stock pass receipt.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{body_index::ExpressionKind, parse::ParseKey},
};
pub(super) const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
pub(super) fn setup(text: &str, edition: u16) -> (Database, PreparedSourceView, SourceDiscovery) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("lexical-numeric-original.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let unit = Digest::of_bytes(b"LRR lexical numeric source");
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
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"LRR lexical numeric discovery"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR lexical numeric declarative"),
        Digest::of_bytes(b"LRR lexical numeric compiler"),
        std::sync::Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                .unwrap(),
        ),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            _ => panic!("lexical source discovery requires unsupported authority"),
        }
    }
    (db, view, discovery)
}
#[test]
fn unchanged_original_local_headers_admit_exact_bodies_without_child_body_queries_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, discovery) = setup(ORIGINAL, edition);
        let macros =
            crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
                &view, &discovery,
            )
            .unwrap();
        let contexts = macros.contexts().unwrap();
        let main = view
            .outline()
            .items
            .iter()
            .find(|row| row.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(machine)) = crate::engine::run::RunMachine::start_with_discovery(
            &mut db,
            main.clone(),
            100000,
            discovery,
        ) else {
            panic!()
        };
        let context = contexts
            .iter()
            .find(|context| context.source().selection() == view.selection())
            .unwrap();
        let parent = view
            .outline()
            .items
            .iter()
            .find(|row| row.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let source_revision = db.source_revision();
        for name in [
            "id_i8", "id_i16", "id_i32", "id_i64", "id_uint", "id_u8", "id_u16", "id_u32", "id_u64",
        ] {
            let call = format!("{name}(j)");
            let at = u32::try_from(ORIGINAL.find(&call).unwrap()).unwrap() - parent.range.start;
            let node = machine
                .frame
                .body
                .nodes()
                .iter()
                .find(|node| {
                    node.range.start == at
                        && node.range.end == at + u32::try_from(call.len()).unwrap()
                })
                .unwrap();
            assert_eq!(
                node.role,
                crate::syntax::body_index::NodeRole::Expression(ExpressionKind::Call)
            );
            let key = machine
                .frame
                .body
                .pc(node.id, crate::ir::pc::Phase::Entry)
                .unwrap();
            let QueryPoll::Ready(Ok(Some(callee))) = context.bind_lexical_numeric_function(
                &mut db,
                machine.graph.as_ref().unwrap(),
                &machine.frame.body,
                &key,
                name,
                machine.runtime_no_item_proofs.values(),
            ) else {
                panic!("original lexical header did not admit")
            };
            assert_eq!(callee.definition().declared_name, name);
            assert!(callee.signature().scope.owner.is_some());
            callee.revalidate(&mut db).unwrap();
            let child = callee.body().unwrap();
            assert_eq!(
                Digest::of_bytes(
                    &ORIGINAL.as_bytes()[child.range.start as usize..child.range.end as usize]
                ),
                child.body_hash
            );
            assert_eq!(db.source_revision(), source_revision);
            assert!(db.trace.events.iter().all(|event| match &event.key {
                QueryKey::BodyStructure(body) => body == &main,
                QueryKey::Thunk(_) => false,
                _ => true,
            }));
        }
        assert_eq!(machine.opcode_counters().unwrap().operations_started, 0);
        db.finish_run().unwrap();
        // Explicit captured-call-site component setup only; no original let or
        // child body was executed, and this is not a full original pass.
    }
}
#[test]
fn source_alias_generic_value_shadow_and_ambiguity_refuse_before_child_body_or_argument_queries() {
    for text in [
        "type i8=bool;fn main(){fn id(n:i8)->i8{n}id(side());}fn side()->i8{missing();}",
        "fn main(){fn id<T>(n:T)->T{n}id(side());}fn side()->i8{missing();}",
        "fn main(){fn id(n:i8)->i8{n}let id=1i8;id(side());}fn side()->i8{missing();}",
        "fn main(){fn id(n:i8)->i8{n}fn id(n:i16)->i16{n}id(side());}fn side()->i8{missing();}",
    ] {
        let (mut db, view, discovery) = setup(text, 2024);
        let macros =
            crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
                &view, &discovery,
            )
            .unwrap();
        let contexts = macros.contexts().unwrap();
        let main = view
            .outline()
            .items
            .iter()
            .find(|row| row.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(machine)) = crate::engine::run::RunMachine::start_with_discovery(
            &mut db,
            main.key(),
            100000,
            discovery,
        ) else {
            panic!()
        };
        let at = u32::try_from(text.find("id(side())").unwrap()).unwrap() - main.range.start;
        let node = machine
            .frame
            .body
            .nodes()
            .iter()
            .find(|node| node.range.start == at && node.range.end == at + 10)
            .unwrap();
        let key = machine
            .frame
            .body
            .pc(node.id, crate::ir::pc::Phase::Entry)
            .unwrap();
        let context = contexts
            .iter()
            .find(|context| context.source().selection() == view.selection())
            .unwrap();
        let QueryPoll::Ready(Err(error)) = context.bind_lexical_numeric_function(
            &mut db,
            machine.graph.as_ref().unwrap(),
            &machine.frame.body,
            &key,
            "id",
            machine.runtime_no_item_proofs.values(),
        ) else {
            panic!("incomplete source identity was admitted")
        };
        assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body == &main.key(),
            QueryKey::Thunk(_) => false,
            _ => true,
        }));
        assert_eq!(machine.opcode_counters().unwrap().operations_started, 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn absent_entered_context_refuses_before_same_named_module_body_or_arguments() {
    for text in [
        "fn id()->i8{panic!(\"wrong module entered\");}fn main(){fn id()->i8{2i8}id();}",
        "fn id(n:i8)->i8{panic!(\"wrong module entered\");}fn main(){fn id(n:i8)->i8{n}id(3i8);}",
    ] {
        let (mut db, view, discovery) = setup(text, 2024);
        let main = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_discovery(&mut db, main.clone(), 100000, discovery)
        else {
            panic!()
        };
        let RunPoll::Fault(error) = machine.poll(&mut db) else {
            panic!("missing entered context selected a callable")
        };
        assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
        assert_eq!(
            error.message,
            "lexical callee requires exact entered source context"
        );
        assert_eq!(machine.opcode_counters().unwrap().started_by_opcode[0], 0);
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) | QueryKey::Thunk(crate::ir::pc::PcKey { body, .. }) =>
                body == &main,
            _ => true,
        }));
        db.finish_run().unwrap();
    }
}
