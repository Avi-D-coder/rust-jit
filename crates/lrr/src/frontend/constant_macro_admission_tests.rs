//! Nested under the genuine source-admission fixture helper; no guest execution.
use super::*;
use crate::frontend::constant_macro_admission::{AdmittedConstantPanic, CurrentConstantMacroProof};
use crate::syntax::pure_constants::Node;
const PANIC: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/const_panic.rs"
);
const ARRAY: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-66693-panic-in-array-len.rs"
);
const SHADOW_BUILTIN: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/shadow_builtin_macros.rs"
);
const AMBIGUOUS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/ambiguous-panic-rename-panics.rs"
);
fn macro_range(view: &PreparedSourceView, definition: Digest) -> SourceRange {
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.fingerprint() == definition)
        .unwrap();
    let body = item.body.as_ref().unwrap();
    let proofs = view.entered_macro_facts(&body.key()).unwrap();
    let selected = proofs
        .invocations
        .iter()
        .find(|i| i.range.start >= body.range.start && i.range.end <= body.range.end)
        .unwrap();
    selected.range
}
fn proof(
    text: &str,
    name: &str,
    edition: u16,
) -> (
    Database,
    PreparedSourceView,
    Digest,
    ConstantSourceAdmission,
    AdmittedConstantPanic,
) {
    let (db, view, definition) = world(text, name, edition, true);
    let base = admit(&db, &view, definition).unwrap();
    let handle = base.plan().owner_body().unwrap();
    let range = macro_range(&view, definition);
    let admitted = CurrentConstantMacroProof::capture_current(
        &db,
        &view,
        &base,
        &handle.key(),
        range,
        usize::MAX,
    )
    .unwrap();
    (db, view, definition, base, admitted)
}
fn selected_node(admission: &ConstantSourceAdmission) -> Result<Node, Diagnostic> {
    let (constant, _, length) = admission.plan().entry()?;
    let expr = constant.map(|c| c.body).or(length).unwrap();
    admission.plan().source().node(&expr)
}

#[test]
fn root_evaluator_retains_actual_upstream_panic_message_and_selected_identity() {
    use crate::frontend::constants::{ConstLimits, evaluate};
    for edition in [2015, 2018, 2021] {
        for (name, message) in [
            ("Z", "cheese"),
            ("Z2", "explicit panic"),
            ("Z_CORE", "cheese"),
            ("Z2_CORE", "explicit panic"),
        ] {
            let (db, view, definition, _, proof) = proof(PANIC, name, edition);
            let current = ConstantSourceAdmission::capture_current_with_macro_proofs(
                &db,
                &view,
                definition,
                ConstantRequest::Constant,
                usize::MAX,
                std::slice::from_ref(&proof),
            )
            .unwrap();
            current.revalidate(&db, &view).unwrap();
            let failure = evaluate(current.plan(), ConstLimits::default()).unwrap_err();
            assert_eq!(failure.diagnostic().code, "LRR-E-CONST-PANIC");
            assert_eq!(failure.diagnostic().message, message);
            assert_eq!(failure.range(), proof.range());
            assert_eq!(failure.source(), Digest::of_bytes(PANIC.as_bytes()));
            let panic = failure.panic().expect("source-proven typed constant panic");
            assert_eq!(panic.message(), message);
            assert_eq!(panic.definition(), proof.definition());
            assert_eq!(panic.proof(), proof.identity());
        }
    }
}

#[test]
fn root_missing_source_macro_proof_cannot_become_a_typed_panic() {
    use crate::frontend::constants::{ConstLimits, evaluate};
    let (_, _, _, base, _) = proof(PANIC, "Z", 2015);
    let failure = evaluate(base.plan(), ConstLimits::default()).unwrap_err();
    assert_eq!(failure.diagnostic().code, "LRR-E-CONST-MACRO-SOURCE");
    assert!(failure.panic().is_none());
}
#[test]
fn unchanged_original_qualified_empty_and_string_panics_use_real_messages() {
    // Original declared editions only; 2024 is separately a component extension
    // and must never be relabelled the upstream edition configuration.
    for edition in [2015, 2018, 2021] {
        for (name, message) in [
            ("Z", "cheese"),
            ("Z2", "explicit panic"),
            ("Z_CORE", "cheese"),
            ("Z2_CORE", "explicit panic"),
        ] {
            let (db, view, definition, base, proof) = proof(PANIC, name, edition);
            assert!(matches!(
                selected_node(&base).unwrap(),
                Node::RequiredMacro(_)
            ));
            assert_eq!(proof.message(), message);
            let current = ConstantSourceAdmission::capture_current_with_macro_proofs(
                &db,
                &view,
                definition,
                ConstantRequest::Constant,
                usize::MAX,
                std::slice::from_ref(&proof),
            )
            .unwrap();
            let Node::AdmittedPanic(reached) = selected_node(&current).unwrap() else {
                panic!("missing proven panic node")
            };
            assert_eq!(reached.message(), message);
            assert_eq!(reached.identity(), proof.identity());
            assert_eq!(reached.definition(), proof.definition());
            assert_ne!(current.plan().identity(), base.plan().identity());
            current.revalidate(&db, &view).unwrap();
        }
    }
}
#[test]
fn unchanged_array_invalid_float_argument_is_not_a_canned_panic() {
    for edition in [2015, 2018, 2021] {
        let (db, view, definition) = world(ARRAY, "main", edition, true);
        let range = macro_range(&view, definition);
        let base = ConstantSourceAdmission::capture_current(
            &db,
            &view,
            definition,
            ConstantRequest::ArrayLength(range),
            usize::MAX,
        )
        .unwrap();
        let body = base.plan().owner_body().unwrap();
        let result = CurrentConstantMacroProof::capture_current(
            &db,
            &view,
            &base,
            &body.key(),
            range,
            usize::MAX,
        );
        assert!(result.is_err());
        assert_ne!(result.unwrap_err().code, "LRR-E-CONST-PANIC");
    }
}
#[test]
fn nonstring_operands_and_formatting_remain_exact_current_frontiers() {
    for text in [
        "const X:()=panic!(42);fn main(){}",
        "const X:()=panic!(\"{}\",1);fn main(){}",
        "const X:()=panic!(\"{missing}\");fn main(){}",
    ] {
        let (db, view, definition) = world(text, "X", 2024, true);
        let base = admit(&db, &view, definition).unwrap();
        let range = macro_range(&view, definition);
        let error = CurrentConstantMacroProof::capture_current(
            &db,
            &view,
            &base,
            &base.plan().owner_body().unwrap().key(),
            range,
            usize::MAX,
        )
        .unwrap_err();
        assert_eq!(error.code, "LRR-E-CONST-MACRO-OPERANDS");
        assert!(matches!(
            selected_node(&base).unwrap(),
            Node::RequiredMacro(_)
        ));
    }
}
#[test]
fn wrong_body_range_and_other_definition_cannot_reuse_selected_proof() {
    let (db, view, definition, base, proof) = proof(
        "const X:()=panic!();const Y:()=panic!();fn main(){}",
        "X",
        2024,
    );
    let y = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Y")
        .unwrap();
    let result = ConstantSourceAdmission::capture_current_with_macro_proofs(
        &db,
        &view,
        y.def_key.fingerprint(),
        ConstantRequest::Constant,
        usize::MAX,
        std::slice::from_ref(&proof),
    );
    // Same module proof may be retained for its own exact X range, but cannot
    // authorize the reached Y macro. Source tables may include forward constants.
    let current = result.unwrap();
    let Node::RequiredMacro(site) = selected_node(&current).unwrap() else {
        panic!("foreign X proof authorized Y")
    };
    assert_eq!(site.body(), &y.body.as_ref().unwrap().key());
    let failure = crate::frontend::constants::evaluate(
        current.plan(),
        crate::frontend::constants::ConstLimits::default(),
    )
    .unwrap_err();
    assert_eq!(failure.diagnostic().code, "LRR-E-CONST-MACRO-SOURCE");
    assert!(failure.panic().is_none());
    let range = SourceRange {
        start: proof.range().start + 1,
        end: proof.range().end,
    };
    assert!(
        CurrentConstantMacroProof::capture_current(
            &db,
            &view,
            &base,
            &base.plan().owner_body().unwrap().key(),
            range,
            usize::MAX
        )
        .is_err()
    );
    assert!(
        ConstantSourceAdmission::capture_current_with_macro_proofs(
            &db,
            &view,
            definition,
            ConstantRequest::Constant,
            usize::MAX,
            &[proof.clone(), proof]
        )
        .is_err()
    );
}
#[test]
fn macro_proof_currentness_catches_epoch_without_source_revision_change() {
    let (mut db, view, _, _, proof) = proof("const X:()=panic!();fn main(){}", "X", 2024);
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), revision);
    assert!(proof.revalidate(&db, &view).is_err());
}
#[test]
fn copied_source_and_deleted_compiler_facts_cannot_authorize_macro() {
    let (mut db, view, _, _, proof) = proof("const X:()=panic!();fn main(){}", "X", 2024);
    let (_, other, _) = world(
        "const X:()=panic!(\"changed\");fn main(){}",
        "X",
        2024,
        true,
    );
    assert!(proof.revalidate(&db, &other).is_err());
    let mut facts = view.prepared_inputs().clone();
    facts.remove(&InputKey::SourceDependencies);
    db.publish_inputs(facts).unwrap();
    assert!(proof.revalidate(&db, &view).is_err());
}
#[test]
fn user_named_panic_never_matches_a_standard_facet_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = "macro_rules! panic{()=>{()}}const X:()=panic!();fn main(){}";
        // Structural original hygiene/lookup proves the user identity even when
        // its expansion/NoItem publication remains an external prerequisite.
        let unit = Digest::of_bytes(b"LRR user panic source");
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let parsed = crate::syntax::parse::ParsedFile::parse(&blob, profile.clone()).unwrap();
        let outline = parsed
            .outline(unit, vec![], &CfgSet::guest_default())
            .unwrap();
        let root = crate::expand::source_hygiene::source_root(unit, edition).unwrap();
        let facts = crate::frontend::hygiene::HygieneFacts::new(root.contexts, root.rows)
            .unwrap()
            .with_name_contexts(root.names)
            .unwrap();
        let index = crate::frontend::source_macros::SourceMacroIndex::from_outline(
            &outline,
            root.context,
            facts,
        )
        .unwrap();
        let lookup = index
            .lookup(&[], text.find("const X").unwrap() as u64, "panic")
            .unwrap();
        let crate::frontend::macro_scopes::HygienicLookup::Selection(
            crate::frontend::macro_origins::MacroSelection::Defined(definition),
        ) = lookup
        else {
            panic!("user panic identity unestablished")
        };
        let catalog =
            crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(&profile)
                .unwrap();
        assert_eq!(catalog.select(&definition).unwrap(), None);
    }
}
#[test]
fn unchanged_shadow_and_ambiguous_sources_are_preserved_as_prerequisites() {
    // The unchanged files include auxiliary/generated/item macros and ambiguity;
    // this bounded original constant lane cannot fabricate their expansion/seal.
    for text in [SHADOW_BUILTIN, AMBIGUOUS] {
        let profile = ProfileIdentity::current();
        let parsed = crate::syntax::parse::ParsedFile::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            profile,
        )
        .unwrap();
        let outline = parsed
            .outline(
                Digest::of_bytes(b"official macro prerequisite"),
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap();
        assert!(!outline.imports.is_empty() || !outline.item_invocations.is_empty());
    }
}

#[test]
fn renamed_standard_panic_uses_the_selected_identity_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = "use std::panic as p;const X:()=p!(\"renamed\");fn main(){}";
        let (db, view, definition, _, proof) = proof(text, "X", edition);
        assert_eq!(proof.message(), "renamed");
        let admitted = ConstantSourceAdmission::capture_current_with_macro_proofs(
            &db,
            &view,
            definition,
            ConstantRequest::Constant,
            usize::MAX,
            std::slice::from_ref(&proof),
        )
        .unwrap();
        let Node::AdmittedPanic(selected) = selected_node(&admitted).unwrap() else {
            panic!("renamed source was not selected")
        };
        assert_eq!(selected.definition(), proof.definition());
        assert_eq!(selected.message(), "renamed");
    }
}
#[test]
fn unchanged_lexical_user_shadow_has_exact_cold_source_selection_and_expansion() {
    for edition in [2015, 2018, 2021, 2024] {
        preparation::original_m4_selection(edition);
    }
}
#[test]
fn genuine_other_intrinsic_no_item_proof_does_not_certify_panic_semantics() {
    let (db, view, definition) = world("const X:()=assert!(true);fn main(){}", "X", 2024, true);
    let base = admit(&db, &view, definition).unwrap();
    let range = macro_range(&view, definition);
    let error = CurrentConstantMacroProof::capture_current(
        &db,
        &view,
        &base,
        &base.plan().owner_body().unwrap().key(),
        range,
        usize::MAX,
    )
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-CONST-MACRO-SOURCE");
    assert!(matches!(
        selected_node(&base).unwrap(),
        Node::RequiredMacro(_)
    ));
}

#[test]
fn root_current_macro_failure_cache_retains_actual_message_and_selected_proof() {
    use crate::{
        db::{key::QueryKey, value::QueryValue},
        frontend::constants::{ConstLimits, query::fetch_current_constant, wire::ConstantWireType},
        ir::op::PrimitiveType,
    };
    for edition in [2015, 2018, 2021] {
        let (mut db, view, definition, _, selected) = proof(PANIC, "Z", edition);
        let current = ConstantSourceAdmission::capture_current_with_macro_proofs(
            &db,
            &view,
            definition,
            ConstantRequest::Constant,
            usize::MAX,
            std::slice::from_ref(&selected),
        )
        .unwrap();
        let ty = ConstantWireType::Scalar(PrimitiveType::Unit);
        let first =
            fetch_current_constant(&mut db, &view, &current, &ty, ConstLimits::default(), 1024)
                .into_ready()
                .unwrap()
                .unwrap_err();
        assert_eq!(first.code, "LRR-E-CONST-PANIC");
        assert_eq!(first.message, "cheese");
        let key = QueryKey::Constant(
            crate::db::key::ConstantKey::capture(current.plan(), &ty, current.identity())
                .unwrap()
                .bind_universe(db.constant_input_universe().unwrap()),
        );
        let memo = db.memo(&key).unwrap();
        let QueryValue::ConstantFailure(value) = memo.value.as_ref() else {
            panic!("actual source macro failure not typed")
        };
        let panic = value.panic().expect("source-selected macro panic");
        assert_eq!(panic.message(), "cheese");
        assert_eq!(panic.definition(), selected.definition());
        assert_eq!(panic.proof(), selected.identity());
        assert!(value.numeric_panic().is_none());
        assert_eq!(value.range(), selected.range());
        let second =
            fetch_current_constant(&mut db, &view, &current, &ty, ConstLimits::default(), 1024)
                .into_ready()
                .unwrap()
                .unwrap_err();
        assert_eq!(second.message, first.message);
        assert_eq!(db.trace.counts["Constant:ProviderRun"], 1);
        assert_eq!(db.trace.counts["Constant:MemoHit"], 1);
    }
}

#[path = "constant_preparation_tests.rs"]
mod preparation;
