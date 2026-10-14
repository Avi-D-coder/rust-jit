//! Source/header component controls; no initializer, callee or length evaluation.
use super::*;
use crate::{
    core::{digest::StableEncode, profile::ProfileIdentity},
    db::{
        dependency::{InputKey, InputValue},
        key::OutlineKey,
    },
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{constant_source_admission::ConstantRequest, constants::ConstValue},
    source::{
        changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const NESTED: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-nested.rs");
const DIV_ZERO: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/const-eval-query-stack.rs"
);
const PANIC: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/const_panic.rs"
);
const FUNCTIONS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn.rs");
const MODULE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/module-const-array-size-13763.rs"
);
const ARRAY_MISMATCH: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/array-literal-len-mismatch.rs"
);
const ARRAY_OOB: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-array-oob.rs");
const BAD_LENGTH_TYPES: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-69310-array-size-lit-wrong-ty.rs"
);
fn world(text: &str, name: &str, edition: u16) -> (Database, PreparedSourceView, Digest) {
    let unit = Digest::of_bytes(b"LRR result type readiness acceptance");
    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("result-type-original.rs").unwrap();
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
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                    "actual discovery incomplete: {} {}",
                    diagnostic.code, diagnostic.message
                ),
                _ => panic!("result source namespace requires an unestablished preparation input"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    (db, view, definition)
}
fn source_admit(
    db: &Database,
    view: &PreparedSourceView,
    definition: Digest,
    request: ConstantRequest,
) -> Result<ConstantSourceAdmission, Diagnostic> {
    ConstantSourceAdmission::capture_current(db, view, definition, request, MAX_BYTES)
}
fn ready(
    db: &Database,
    view: &PreparedSourceView,
    definition: Digest,
    request: ConstantRequest,
) -> Result<CurrentConstantResultType, Diagnostic> {
    let admitted = source_admit(db, view, definition, request)?;
    CurrentConstantResultType::capture_current(db, view, admitted, MAX_BYTES)
}
fn arg(value: u64) -> ConstValue {
    ConstValue::from_scalar(
        crate::runtime::value::ValueRepr::unsigned(
            crate::runtime::shape::PrimitiveIdentity::from_recipe(PrimitiveType::U32)
                .unwrap()
                .id(),
            value,
        )
        .unwrap(),
    )
    .unwrap()
}
fn array(primitive: PrimitiveType, length: u64) -> ConstantWireType {
    ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(primitive)),
        length,
    }
}
#[test]
fn unchanged_named_scalar_header_all_editions_without_initializing() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(NESTED, "X", edition);
        let before = db.trace.events.len();
        let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        let ExpectedResultReadiness::ReadyScalar(expected) = proof
            .check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::U32))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(expected.primitive(), PrimitiveType::U32);
        assert_eq!(expected.source_identity(), proof.identity());
        assert_eq!(
            expected.wire_type().fingerprint_checked().unwrap(),
            ConstantWireType::Scalar(PrimitiveType::U32)
                .fingerprint_checked()
                .unwrap()
        );
        assert_eq!(proof.admission().plan().owner_definition(), definition);
        assert_eq!(proof.admission().plan().profile().edition, edition);
        assert_eq!(before, db.trace.events.len());
        assert!(proof.metadata_bytes() <= MAX_BYTES);
        assert!(
            proof
                .check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::Bool))
                .is_err()
        );
    }
}
#[test]
fn unchanged_module_scalar_owns_source_scope_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(MODULE, "BITS", edition);
        let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        assert_eq!(proof.scalar().unwrap().primitive(), PrimitiveType::Usize);
        assert_eq!(
            proof.admission().plan().owner_context().scope.module_path,
            vec!["u8"]
        );
        proof.revalidate(&db, &view).unwrap();
    }
}
#[test]
fn unchanged_const_fn_result_binds_current_signature_and_each_argument_packet() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(FUNCTIONS, "add", edition);
        let before = db.trace.events.len();
        let a = ready(
            &db,
            &view,
            definition,
            ConstantRequest::ConstCall(vec![arg(44), arg(22)]),
        )
        .unwrap();
        let b = ready(
            &db,
            &view,
            definition,
            ConstantRequest::ConstCall(vec![arg(22), arg(44)]),
        )
        .unwrap();
        assert_eq!(a.scalar().unwrap().primitive(), PrimitiveType::U32);
        assert_ne!(a.identity(), b.identity());
        assert_ne!(
            a.admission().plan().identity(),
            b.admission().plan().identity()
        );
        assert_eq!(
            a.check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::Bool))
                .unwrap_err()
                .code,
            "LRR-E-CONST-TYPE"
        );
        assert_eq!(before, db.trace.events.len());
    }
}
#[test]
fn unchanged_array_mismatch_retains_exact_header_demand_not_initializer_size() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(ARRAY_MISMATCH, "NUMBERS", edition);
        let before = db.trace.events.len();
        let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        let (element, demand) = proof.array().unwrap();
        assert_eq!(element.primitive(), PrimitiveType::U8);
        assert_eq!(demand.definition(), definition);
        assert_eq!(
            &ARRAY_MISMATCH[demand.range().start as usize..demand.range().end as usize],
            "3"
        );
        assert_eq!(demand.source(), view.selection());
        assert_eq!(demand.context(), proof.admission().plan().owner_context());
        assert_eq!(Some(demand.body()), proof.admission().plan().owner_body());
        assert_eq!(
            demand.parent_plan_identity(),
            proof.admission().plan().identity()
        );
        assert_eq!(demand.identity(), proof.identity());
        assert!(
            matches!(demand.request(),ConstantRequest::ArrayLength(range) if range==demand.range())
        );
        assert_eq!(
            demand.expected_wire().fingerprint_checked().unwrap(),
            ConstantWireType::Scalar(PrimitiveType::Usize)
                .fingerprint_checked()
                .unwrap()
        );
        // Even the initializer's count2 cannot prove the written type's count.
        for caller_count in [0, 2, 3, 99] {
            assert!(
                matches!(proof.check_current_expected(&db,&view,&array(PrimitiveType::U8,caller_count)).unwrap(),ExpectedResultReadiness::RequiredArrayLength(actual) if actual.range()==demand.range())
            );
        }
        assert_eq!(before, db.trace.events.len());
        // No E0308/static acceptance or initializer evaluation is claimed.
    }
}
#[test]
fn unchanged_oob_length_is_demanded_without_touching_referenced_constant_or_initializer() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(ARRAY_OOB, "BLUB", edition);
        let before = db.trace.events.len();
        let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        let (_, demand) = proof.array().unwrap();
        assert_eq!(
            &ARRAY_OOB[demand.range().start as usize..demand.range().end as usize],
            "FOO[4]"
        );
        assert!(matches!(
            proof
                .check_current_expected(&db, &view, &array(PrimitiveType::U32, 2))
                .unwrap(),
            ExpectedResultReadiness::RequiredArrayLength(_)
        ));
        assert_eq!(before, db.trace.events.len());
    }
}
#[test]
fn unchanged_wrong_length_operand_types_never_fabricate_a_usize_value() {
    for name in ["A", "B"] {
        let (db, view, definition) = world(BAD_LENGTH_TYPES, name, 2024);
        let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        let (element, demand) = proof.array().unwrap();
        assert_eq!(element.primitive(), PrimitiveType::Unit);
        let expected = if name == "A" { "0.1" } else { "b\"a\"" };
        assert_eq!(
            &BAD_LENGTH_TYPES[demand.range().start as usize..demand.range().end as usize],
            expected
        );
        assert!(matches!(
            proof
                .check_current_expected(&db, &view, &array(PrimitiveType::Unit, 1))
                .unwrap(),
            ExpectedResultReadiness::RequiredArrayLength(_)
        ));
    }
}
#[test]
fn known_shape_and_element_mismatch_refuse_before_any_length_work() {
    let (db, view, definition) = world(ARRAY_OOB, "BLUB", 2024);
    let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let before = db.trace.events.len();
    for expected in [
        ConstantWireType::Scalar(PrimitiveType::U32),
        array(PrimitiveType::Bool, 2),
        ConstantWireType::Array {
            element: Box::new(array(PrimitiveType::U32, 1)),
            length: 2,
        },
    ] {
        assert_eq!(
            proof
                .check_current_expected(&db, &view, &expected)
                .unwrap_err()
                .code,
            "LRR-E-CONST-TYPE"
        );
    }
    assert_eq!(before, db.trace.events.len());
    let (db, view, definition) = world(NESTED, "X", 2024);
    let before = db.trace.events.len();
    let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    assert_eq!(
        proof
            .check_current_expected(&db, &view, &array(PrimitiveType::U32, 1))
            .unwrap_err()
            .code,
        "LRR-E-CONST-TYPE"
    );
    assert_eq!(before, db.trace.events.len());
}
#[test]
fn known_scalar_mismatch_precedes_reached_panic_or_unsupported_callee_frontiers() {
    for source in [
        "const X:u32=panic!();fn main(){}",
        "const X:u32=not_a_const_fn();fn main(){}",
    ] {
        let (db, view, definition) = world(source, "X", 2024);
        let before = db.trace.events.len();
        let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        assert_eq!(
            proof
                .check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::Bool))
                .unwrap_err()
                .code,
            "LRR-E-CONST-TYPE"
        );
        assert_eq!(before, db.trace.events.len());
    }
}
#[test]
fn unsupported_alias_generic_and_associated_result_types_grant_no_scalar_authority() {
    for source in [
        "type i32=bool;const X:i32=true;fn main(){}",
        "type Alias=u32;const X:Alias=1;fn main(){}",
        "mod m{pub type Alias=u32;}const X:m::Alias=1;fn main(){}",
        "const X:[[u8;1];1]=[[1]];fn main(){}",
    ] {
        let (db, view, definition) = world(source, "X", 2024);
        assert!(ready(&db, &view, definition, ConstantRequest::Constant).is_err());
    }
    let (db, view, definition) = world(FUNCTIONS, "generic", 2024);
    assert!(
        ready(
            &db,
            &view,
            definition,
            ConstantRequest::ConstCall(vec![arg(1)])
        )
        .is_err()
    );
}
#[test]
fn array_const_fn_demand_preserves_arguments_and_admits_independent_header_length() {
    let source = "const fn f(x:u32)->[u8;2]{let _=x;[1,2]}fn main(){}";
    let (db, view, definition) = world(source, "f", 2024);
    let proof = ready(
        &db,
        &view,
        definition,
        ConstantRequest::ConstCall(vec![arg(7)]),
    )
    .unwrap();
    let (_, demand) = proof.array().unwrap();
    assert_eq!(
        &source[demand.range().start as usize..demand.range().end as usize],
        "2"
    );
    assert!(matches!(
        proof
            .check_current_expected(&db, &view, &array(PrimitiveType::U8, 2))
            .unwrap(),
        ExpectedResultReadiness::RequiredArrayLength(_)
    ));
    // Exact header literal is independent of the scalar parameter/body local.
    // No initializer/const-call evaluation or guessed lexical argument is needed.
    let before = db.trace.counts.clone();
    let child = source_admit(&db, &view, definition, demand.request()).unwrap();
    child.revalidate(&db, &view).unwrap();
    assert_eq!(child.plan().range(), demand.range());
    assert_eq!(db.trace.counts, before);
}
#[test]
fn private_scalar_and_length_tamper_controls_fail_against_exact_admitted_header() {
    let (db, view, definition) = world(NESTED, "X", 2024);
    let mut proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let ResultReadiness::Scalar(scalar) = &mut proof.readiness else {
        panic!()
    };
    scalar.primitive = PrimitiveType::Bool;
    assert!(
        proof
            .check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::Bool))
            .is_err()
    );
    let (db, view, definition) = world(ARRAY_MISMATCH, "NUMBERS", 2024);
    let mut proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let ResultReadiness::Array { length, .. } = &mut proof.readiness else {
        panic!()
    };
    length.range.start += 1;
    assert!(proof.revalidate(&db, &view).is_err());
}
#[test]
fn stale_derived_epoch_or_public_header_body_pair_refuses_readiness() {
    let (mut db, view, definition) = world(NESTED, "X", 2024);
    let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(revision, db.source_revision());
    assert!(
        proof
            .check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::U32))
            .is_err()
    );
    let (mut db, view, definition) = world(NESTED, "X", 2024);
    let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let mut facts = view.prepared_inputs().clone();
    let InputValue::DefinitionBody(body) = facts
        .get_mut(&InputKey::DefinitionBody(definition))
        .unwrap()
    else {
        panic!()
    };
    body.range.end -= 1;
    db.publish_inputs(facts).unwrap();
    assert!(proof.revalidate(&db, &view).is_err());
}
#[test]
fn tiny_metadata_budget_rejects_before_attaching_result_or_running_query_work() {
    let (db, view, definition) = world(NESTED, "X", 2024);
    let admitted = source_admit(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let before = db.trace.events.len();
    assert_eq!(
        CurrentConstantResultType::capture_current(&db, &view, admitted, 1)
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
    assert_eq!(before, db.trace.events.len());
}

#[test]
fn unchanged_divide_zero_and_panic_header_mismatch_has_no_provider_or_initializer_work() {
    for (original, name, editions) in [
        (DIV_ZERO, "X", &[2015, 2018, 2021, 2024][..]),
        (PANIC, "Z", &[2015, 2018, 2021][..]),
    ] {
        for edition in editions {
            let (db, view, definition) = world(original, name, *edition);
            let events = db.trace.events.len();
            let counts = db.trace.counts.clone();
            let proof = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
            assert_eq!(
                proof
                    .check_current_expected(
                        &db,
                        &view,
                        &ConstantWireType::Scalar(PrimitiveType::Bool)
                    )
                    .unwrap_err()
                    .code,
                "LRR-E-CONST-TYPE"
            );
            assert_eq!(db.trace.events.len(), events);
            assert_eq!(db.trace.counts, counts);
        }
    }
}
#[test]
fn exact_header_length_request_has_intrinsic_usize_role_without_evaluation_or_name_recovery() {
    for (source, name) in [
        (ARRAY_MISMATCH, "NUMBERS"),
        ("type usize=bool;const X:[u8;3]=[1,2,3];fn main(){}", "X"),
    ] {
        let (db, view, definition) = world(source, name, 2024);
        let parent = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
        let (_, demand) = parent.array().unwrap();
        let length = ready(&db, &view, definition, demand.request()).unwrap();
        assert_eq!(length.scalar().unwrap().primitive(), PrimitiveType::Usize);
        assert_eq!(
            length
                .check_current_expected(&db, &view, &ConstantWireType::Scalar(PrimitiveType::Bool))
                .unwrap_err()
                .code,
            "LRR-E-CONST-TYPE"
        );
        assert!(db.trace.counts.is_empty());
    }
}
#[path = "constant_result_query_tests.rs"]
mod bridge_tests;

#[path = "constant_child_admission_tests.rs"]
mod child_tests;

#[path = "constant_cold_macro_tests.rs"]
mod cold_macro_tests;

#[path = "constant_parameter_length_tests.rs"]
mod parameter_length_tests;
