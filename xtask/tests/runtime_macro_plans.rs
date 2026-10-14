use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    expand::scopes::MacroDefinition,
    facade::macros::RuntimeMacroKind,
    frontend::{
        macro_names::PathLookup,
        runtime_macro_catalog::RuntimeMacroCatalog,
        runtime_macro_plan::{
            MessageTiming, OperandUse, RuntimeMacroPreparation, prepare_runtime_macro,
        },
    },
    source::snapshot::SourceBlob,
    syntax::{macro_calls::SourceMacroCall, parse::ParsedFile, spans::SourceRange},
};
fn invocation(text: &str) -> (ParsedFile, SourceMacroCall) {
    let source = format!("fn case(){{{text};}}");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let call = parsed
        .macro_call_at(SourceRange {
            start: 10,
            end: 10 + text.len() as u32,
        })
        .unwrap();
    (parsed, call)
}
fn compiler(catalog: &RuntimeMacroCatalog, kind: RuntimeMacroKind) -> MacroDefinition {
    catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap() == Some(kind))
        .unwrap()
        .clone()
}
#[test]
fn resolved_runtime_macro_plans_capture_operands_and_defer_assertion_messages() {
    let catalog = RuntimeMacroCatalog::new(Digest([182; 32]), 2024).unwrap();
    for (kind, text, count, use_mode, timing) in [
        (
            RuntimeMacroKind::Assert,
            "renamed!(condition(), \"failure {}\", unvisited())",
            1,
            Some(OperandUse::Predicate),
            MessageTiming::OnAssertionFailure,
        ),
        (
            RuntimeMacroKind::AssertEq,
            "renamed!(left(), right(), \"failure {}\", unvisited())",
            2,
            Some(OperandUse::SharedComparison),
            MessageTiming::OnAssertionFailure,
        ),
        (
            RuntimeMacroKind::AssertNe,
            "renamed!(left(), right())",
            2,
            Some(OperandUse::SharedComparison),
            MessageTiming::OnAssertionFailure,
        ),
        (
            RuntimeMacroKind::Panic,
            "renamed!(\"panic {}\", message())",
            0,
            None,
            MessageTiming::Always,
        ),
    ] {
        let (parsed, call) = invocation(text);
        let definition = compiler(&catalog, kind);
        let RuntimeMacroPreparation::Intrinsic(plan) = prepare_runtime_macro(
            &parsed,
            call,
            PathLookup::Defined(definition.clone()),
            &catalog,
        )
        .unwrap() else {
            panic!("builtin definition was not prepared")
        };
        assert_eq!(plan.definition(), &definition);
        assert_eq!(plan.arguments().kind(), kind);
        assert_eq!(plan.arguments().operands().len(), count);
        assert_eq!(plan.operand_use(), use_mode);
        assert_eq!(plan.message_timing(), timing);
        assert_eq!(plan.invocation().key(), plan.arguments().source());
    }
}
#[test]
fn user_macro_named_assert_keeps_its_grammar_and_identity_without_builtin_capture() {
    let catalog = RuntimeMacroCatalog::new(Digest([182; 32]), 2024).unwrap();
    let (parsed, call) = invocation("assert!(custom => grammar; that is not an expression list)");
    let user = MacroDefinition {
        identity: Digest([181; 32]),
        defining_unit: Digest([180; 32]),
        definition_edition: 2024,
    };
    let RuntimeMacroPreparation::SourceExpansion {
        definition,
        invocation,
    } = prepare_runtime_macro(&parsed, call, PathLookup::Defined(user.clone()), &catalog).unwrap()
    else {
        panic!("user macro became a builtin")
    };
    assert_eq!(definition, user);
    assert_eq!(invocation.key(), &parsed.key());
}
#[test]
fn unresolved_runtime_macros_preserve_ambiguity_missing_input_and_visibility() {
    let catalog = RuntimeMacroCatalog::new(Digest([182; 32]), 2024).unwrap();
    let a = compiler(&catalog, RuntimeMacroKind::Assert);
    let b = compiler(&catalog, RuntimeMacroKind::Panic);
    let demand = Digest([179; 32]);
    for lookup in [
        PathLookup::Absent,
        PathLookup::NeedInput(demand),
        PathLookup::Ambiguous(vec![a, b]),
        PathLookup::Inaccessible(vec![]),
    ] {
        let expected = lookup.clone();
        let (parsed, call) = invocation("assert!(not => builtin grammar)");
        let RuntimeMacroPreparation::Resolution { lookup, invocation } =
            prepare_runtime_macro(&parsed, call, lookup, &catalog).unwrap()
        else {
            panic!("resolution outcome was erased")
        };
        assert_eq!(lookup, expected);
        assert_eq!(invocation.key(), &parsed.key());
    }
}
#[test]
fn runtime_macro_plans_refuse_stale_source_forged_definitions_and_bad_builtin_grammar() {
    let catalog = RuntimeMacroCatalog::new(Digest([182; 32]), 2024).unwrap();
    let definition = compiler(&catalog, RuntimeMacroKind::Assert);
    let (parsed, call) = invocation("assert!()");
    assert!(
        prepare_runtime_macro(
            &parsed,
            call,
            PathLookup::Defined(definition.clone()),
            &catalog
        )
        .is_err()
    );
    let (parsed, call) = invocation("assert!(true)");
    let mut forged = definition.clone();
    forged.defining_unit = Digest([180; 32]);
    assert!(prepare_runtime_macro(&parsed, call, PathLookup::Defined(forged), &catalog).is_err());
    let (_, call) = invocation("assert!(false)");
    assert!(
        prepare_runtime_macro(&parsed, call, PathLookup::Defined(definition), &catalog).is_err()
    );
}
