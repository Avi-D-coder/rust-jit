//! Test-only observation of the ordinary interpreter, never another executor.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll},
    ir::{op::PrimitiveType, pc::SlotKind},
    runtime::shape::PrimitiveIdentity,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    traits::term::{ParamKey, TyTerm},
};
use std::cell::RefCell;
#[derive(Debug)]
struct Observation {
    definition: Digest,
    kind: SlotKind,
    source: TyTerm,
    runtime: Option<TypeId>,
}
thread_local! { static OBSERVATIONS:RefCell<Option<Vec<Observation>>>=const {RefCell::new(None)}; }
/// Disabled unless this test explicitly records. It observes source metadata,
/// performs no query and retains no guest values, references or continuations.
pub(super) fn observe(frame: &Frame, slot: SlotId) {
    OBSERVATIONS.with(|trace| {
        let mut trace = trace.borrow_mut();
        let Some(trace) = trace.as_mut() else {
            return;
        };
        let Some(source) = frame.source_view_ref(slot).unwrap() else {
            return;
        };
        let mut leaf = source;
        while let TyTerm::Ref(_, _, target) = leaf {
            leaf = target;
        }
        let runtime = match leaf {
            TyTerm::Param(key) => frame
                .context
                .as_ref()
                .and_then(|context| context.runtime_binding(*key)),
            _ => None,
        };
        trace.push(Observation {
            definition: frame.body.body().definition,
            kind: frame.body.slot_kind(slot).unwrap(),
            source: source.clone(),
            runtime,
        });
    });
}
struct Recording;
impl Drop for Recording {
    fn drop(&mut self) {
        OBSERVATIONS.with(|trace| {
            trace.borrow_mut().take();
        });
    }
}
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("{error}"),
        QueryPoll::NeedInput(input) => panic!("{input:?}"),
    }
}
fn depth(source: &TyTerm, parameter: ParamKey) -> Option<usize> {
    match source {
        TyTerm::Param(key) if *key == parameter => Some(0),
        TyTerm::Ref(_, _, target) => depth(target, parameter).map(|n| n + 1),
        _ => None,
    }
}
const ORIGINAL: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/issue-46095.rs");
const ROUTE: &str = "fn source_route<T:Copy>(x:T) {let alias=x;let r=&alias;let s=&r;let z=**s;let _=z;} fn main(){source_route(7_i32);}";
#[test]
fn ordinary_generic_parameter_alias_borrow_and_deref_keep_rigid_source_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, name, route, primitive) in [
            (ORIGINAL, "identity", false, PrimitiveType::Usize),
            (ROUTE, "source_route", true, PrimitiveType::I32),
        ] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
            let unit = Digest::of_bytes(b"symbolic-parameter-place-control");
            let path = SourcePath::new("symbolic-place-control.rs").unwrap();
            let cfg = CfgSet::guest_default();
            let selection = SourceSelection {
                path: path.clone(),
                outline: crate::db::key::OutlineKey {
                    parse: crate::syntax::parse::ParseKey {
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
            let mut db = Database::default();
            inputs.publish(&mut db).unwrap();
            let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
                panic!()
            };
            db.begin_expansion_epoch(1).unwrap();
            let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
                vec![view.share()],
                view.outline().root_scope.clone(),
                crate::source::manifest::Namespace::Value,
                unit,
                1,
            )
            .unwrap()
            .with_declarative_compiler(
                Digest::of_bytes(b"symbolic-place-source-control-compiler"),
                crate::expand::source_declarative::engine_version(),
                Arc::new(
                    crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                        &view.outline().profile,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
            assert!(matches!(
                discovery.poll().unwrap(),
                crate::expand::source_discovery::SourceDiscoveryStep::Sealed(_)
            ));
            discovery.publish(&mut db).unwrap();
            let target = view
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == name)
                .unwrap()
                .def_key
                .fingerprint();
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
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, main, 0, discovery,
            ));
            let _recording = Recording;
            OBSERVATIONS.with(|trace| assert!(trace.borrow_mut().replace(vec![]).is_none()));
            match run.poll(&mut db) {
                RunPoll::Completed(_) => {}
                RunPoll::Fault(error) => panic!("{edition} {name}: {error}"),
                RunPoll::NeedInput(input) => panic!("{edition} {name} needs {input:?}"),
                RunPoll::Panicked(_) => panic!("{edition} {name}: unexpected guest panic"),
            };
            let observed = OBSERVATIONS.with(|trace| trace.borrow_mut().take().unwrap());
            let parameter = ParamKey {
                definition: target,
                ordinal: 0,
            };
            let concrete = PrimitiveIdentity::from_recipe(primitive).unwrap().id();
            let mut seen = Vec::new();
            for row in observed.iter().filter(|row| row.definition == target) {
                if let Some(n) = depth(&row.source, parameter) {
                    assert_eq!(row.runtime, Some(concrete));
                    seen.push((row.kind, n));
                }
            }
            // `identity` reads and returns its parameter; it never takes its
            // address. Require only the operations the unchanged source uses.
            assert!(
                seen.contains(&(SlotKind::Value, 0)),
                "{edition} {name}: {observed:?}"
            );
            if route {
                for expected in [
                    (SlotKind::Place, 0),
                    (SlotKind::Place, 1),
                    (SlotKind::Place, 2),
                    (SlotKind::Value, 0),
                    (SlotKind::Value, 1),
                    (SlotKind::Value, 2),
                ] {
                    assert!(
                        seen.contains(&expected),
                        "{edition}: missing {expected:?}: {observed:?}"
                    );
                }
            }
        }
    }
}
