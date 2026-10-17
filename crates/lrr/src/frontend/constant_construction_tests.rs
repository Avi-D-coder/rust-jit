//! Actual current source/query joins; no full upstream guest-body execution.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{key::OutlineKey, poll::QueryPoll},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        constant_source_admission::ConstantRequest,
        constants::{ConstLimits, query::fetch_current_constant},
    },
    ir::op::PrimitiveType,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
);
pub(crate) fn world(
    text: &str,
    name: &str,
    edition: u16,
) -> (Database, PreparedSourceView, Digest) {
    let unit = Digest::of_bytes(b"LRR constant construction acceptance");
    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("construction-source.rs").unwrap();
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
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap();
    let definition = item.def_key.fingerprint();
    let scope = item.scope.clone();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    for namespace in [Namespace::Value, Namespace::Type] {
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"result type current world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"result type declarative"),
            Digest::of_bytes(b"result type compiler"),
            Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
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
                _ => panic!("result source namespace requires an unestablished preparation input"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    (db, view, definition)
}
pub(crate) fn result(
    db: &Database,
    view: &PreparedSourceView,
    definition: Digest,
) -> CurrentConstantResultType {
    let admission = ConstantSourceAdmission::capture_current(
        db,
        view,
        definition,
        ConstantRequest::Constant,
        usize::MAX,
    )
    .unwrap();
    CurrentConstantResultType::capture_current(db, view, admission, usize::MAX).unwrap()
}
pub(crate) fn query(
    db: &mut Database,
    view: &PreparedSourceView,
    admission: &ConstantSourceAdmission,
    wire: &ConstantWireType,
) -> crate::db::memo::Outcome {
    let QueryPoll::Ready(outcome) =
        fetch_current_constant(db, view, admission, wire, ConstLimits::default(), 1 << 20)
    else {
        panic!("prepared source unexpectedly suspended")
    };
    outcome.unwrap()
}
pub(crate) fn success(value: &crate::db::memo::Outcome) -> &ConstantQueryValue {
    let QueryValue::Constant(value) = value.value.as_ref() else {
        panic!("query did not publish actual typed constant success")
    };
    value
}
pub(crate) fn array(p: PrimitiveType, n: u64) -> ConstantWireType {
    ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(p)),
        length: n,
    }
}
#[test]
fn unchanged_declared_array_source_length_and_parent_queries_bind_actual_descriptor_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(ORIGINAL, "ARR", edition);
        let result = result(&db, &view, definition);
        let (_, demand) = result.array().unwrap();
        let child = result
            .admission()
            .reselect_array_length_current(&db, &view, demand, usize::MAX)
            .unwrap();
        let length = query(&mut db, &view, &child, &demand.expected_wire());
        assert_eq!(
            success(&length)
                .payload()
                .decode_value()
                .unwrap()
                .scalar()
                .unwrap()
                .payload,
            3
        );
        let parent = query(
            &mut db,
            &view,
            result.admission(),
            &array(PrimitiveType::I32, 3),
        );
        assert!(
            db.memo(&crate::db::key::QueryKey::Constant(
                success(&parent).key().clone()
            ))
            .unwrap()
            .dependencies
            .contains(&crate::db::dependency::DependencyStamp::Query {
                key: crate::db::key::QueryKey::Constant(success(&length).key().clone()),
                fingerprint: length.fingerprint,
            })
        );
        let before = db.trace.events.len();
        let joined = CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &result,
            success(&parent),
            Some((&child, success(&length))),
            usize::MAX,
        )
        .unwrap();
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let bound = joined.bind_guest_descriptor(&mut store).unwrap();
        bound.revalidate(&store).unwrap();
        assert!(
            matches!(store.type_descriptor(bound.ty()).unwrap().key.as_ref(),TypeKey::Array{element,len} if *element==TypeId(6)&&*len==3)
        );
        assert_eq!(
            joined.source_term(),
            &TyTerm::Array(
                Box::new(TyTerm::Primitive(PrimitiveType::I32)),
                ConstTerm::Value(3)
            )
        );
        assert_eq!(store.budget().handles(), 0);
        assert_eq!(before, db.trace.events.len());
    }
}
#[test]
fn source_constructor_requires_real_length_query_not_parent_wire_count() {
    let (mut db, view, definition) = world(ORIGINAL, "ARR", 2024);
    let ready = result(&db, &view, definition);
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &array(PrimitiveType::I32, 3),
    );
    let before = db.trace.events.len();
    assert!(
        CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            None,
            usize::MAX
        )
        .is_err()
    );
    assert_eq!(before, db.trace.events.len());
}
#[test]
fn exact_current_other_length_value_cannot_complete_parent_array_type() {
    let text = "const A:[u8;1]=[25];const B:[u8;2]=[1,2];fn main(){}";
    let (mut db, view, definition) = world(text, "A", 2024);
    let ready = result(&db, &view, definition);
    let bdef = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "B")
        .unwrap()
        .def_key
        .fingerprint();
    let other = result(&db, &view, bdef);
    let (_, d) = other.array().unwrap();
    let child = other
        .admission()
        .reselect_array_length_current(&db, &view, d, usize::MAX)
        .unwrap();
    let length = query(&mut db, &view, &child, &d.expected_wire());
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &array(PrimitiveType::U8, 1),
    );
    assert!(
        CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            Some((&child, success(&length))),
            usize::MAX
        )
        .is_err()
    );
}
#[test]
fn scalar_source_readiness_binds_only_actual_current_success_record() {
    let text = "const A:u8=25;const B:u8=26;fn main(){}";
    let (mut db, view, definition) = world(text, "A", 2024);
    let ready = result(&db, &view, definition);
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &ConstantWireType::Scalar(PrimitiveType::U8),
    );
    let joined = CurrentConstantConstruction::capture_current(
        &db,
        &view,
        &ready,
        success(&parent),
        None,
        usize::MAX,
    )
    .unwrap();
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    assert_eq!(
        joined.bind_guest_descriptor(&mut store).unwrap().ty(),
        TypeId(9)
    );
    let bdef = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "B")
        .unwrap()
        .def_key
        .fingerprint();
    let b = result(&db, &view, bdef);
    let other = query(
        &mut db,
        &view,
        b.admission(),
        &ConstantWireType::Scalar(PrimitiveType::U8),
    );
    assert!(
        CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&other),
            None,
            usize::MAX
        )
        .is_err()
    );
}
#[test]
fn stale_real_source_query_and_descriptor_run_epoch_never_gain_publication_authority() {
    let (mut db, view, definition) = world("const A:u8=25;fn main(){}", "A", 2024);
    let ready = result(&db, &view, definition);
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &ConstantWireType::Scalar(PrimitiveType::U8),
    );
    {
        let joined = CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            None,
            usize::MAX,
        )
        .unwrap();
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let bound = joined.bind_guest_descriptor(&mut store).unwrap();
        store.reset(2).unwrap();
        assert!(bound.revalidate(&store).is_err());
        assert_eq!(store.budget().handles(), 0);
    }
    let old = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(old, db.source_revision());
    assert!(
        CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            None,
            usize::MAX
        )
        .is_err()
    );
}
#[test]
fn source_construction_metadata_limit_precedes_descriptor_or_guest_publication() {
    let (mut db, view, definition) = world("const A:u8=25;fn main(){}", "A", 2024);
    let ready = result(&db, &view, definition);
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &ConstantWireType::Scalar(PrimitiveType::U8),
    );
    let before = db.trace.events.len();
    assert!(
        CurrentConstantConstruction::capture_current(&db, &view, &ready, success(&parent), None, 1)
            .is_err()
    );
    assert_eq!(before, db.trace.events.len());
}
#[test]
fn empty_array_requires_actual_zero_length_and_retains_source_element_identity() {
    let (mut db, view, definition) = world("const A:[u8;0]=[];fn main(){}", "A", 2024);
    let ready = result(&db, &view, definition);
    let (_, d) = ready.array().unwrap();
    let child = ready
        .admission()
        .reselect_array_length_current(&db, &view, d, usize::MAX)
        .unwrap();
    let length = query(&mut db, &view, &child, &d.expected_wire());
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &array(PrimitiveType::U8, 0),
    );
    let joined = CurrentConstantConstruction::capture_current(
        &db,
        &view,
        &ready,
        success(&parent),
        Some((&child, success(&length))),
        usize::MAX,
    )
    .unwrap();
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let bound = joined.bind_guest_descriptor(&mut store).unwrap();
    assert!(
        matches!(store.type_descriptor(bound.ty()).unwrap().key.as_ref(),TypeKey::Array{element,len} if *element==TypeId(9)&&*len==0)
    );
    assert_eq!(store.budget().handles(), 0);
}
#[test]
fn forged_content_only_typed_value_cannot_replace_actual_current_memo_success() {
    use crate::frontend::constants::{ConstValue, memo::ConstMemoPayload};
    use crate::runtime::value::ValueRepr;
    let (mut db, view, definition) = world("const A:u8=25;fn main(){}", "A", 2024);
    let ready = result(&db, &view, definition);
    let wire = ConstantWireType::Scalar(PrimitiveType::U8);
    let parent = query(&mut db, &view, ready.admission(), &wire);
    let key = success(&parent).key();
    let data = ConstValue::from_scalar(ValueRepr::unsigned(TypeId(9), 99).unwrap()).unwrap();
    let payload = ConstMemoPayload::capture_content(
        key,
        &data,
        &wire,
        Digest::of_bytes(b"caller forged dependency content"),
        1024,
    )
    .unwrap();
    let forged = ConstantQueryValue::capture(key, payload).unwrap();
    let before = db.trace.events.len();
    assert!(
        CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            &forged,
            None,
            usize::MAX
        )
        .is_err()
    );
    assert_eq!(before, db.trace.events.len());
    assert_eq!(
        success(&parent)
            .payload()
            .decode_value()
            .unwrap()
            .scalar()
            .unwrap()
            .payload,
        25
    );
}

#[test]
fn original_root_builtin_inert_attribute_needs_no_certificate_for_length_capture() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(ORIGINAL, "ARR", edition);
        let ready = result(&db, &view, definition);
        let (_, demand) = ready.array().unwrap();
        let cold =
            crate::syntax::pure_constants::ConstantSource::capture_prepared(&view, definition)
                .unwrap();
        // `#![allow(dead_code)]` is builtin inert: it is never a macro call, so
        // an uncertified capture plans the same exact length.
        assert_eq!(
            cold.array_length(demand.range()).unwrap().range(),
            demand.range()
        );
        let child = ready
            .admission()
            .reselect_array_length_current(&db, &view, demand, usize::MAX)
            .unwrap();
        assert_eq!(child.plan().range(), demand.range());
        assert_eq!(child.plan().owner_definition(), definition);
    }
}
