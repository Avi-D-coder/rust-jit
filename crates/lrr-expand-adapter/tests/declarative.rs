use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, FailureKind},
    token_wire::Edition,
};
mod fixture {
    include!("support/declarative.rs");
}
use fixture::*;
#[test]
fn pinned_declarative_repetition_preserves_order_and_rejects_cardinality_errors() {
    let rules = DeclarativeMacro::new(definition(
        "($($value:literal),*) => {[$($value),*]};",
        Edition::E2024,
    ))
    .unwrap();
    let args = input("7, 2, 11");
    let result = rules.expand(&args, call_site(&args)).unwrap().unwrap();
    assert_eq!(result.matched_arm, 0);
    assert_eq!(literal_texts(&result.tokens), ["7", "2", "11"]);
    let rules = DeclarativeMacro::new(definition(
        "($($a:literal),*; $($b:literal),*) => {[$(($a,$b)),*]};",
        Edition::E2024,
    ))
    .unwrap();
    let args = input("1, 2; 3");
    let error = match rules.expand(&args, call_site(&args)).unwrap() {
        Err(error) => error,
        Ok(result) => panic!(
            "cardinality mismatch accepted; actual emitted literals: {:?}",
            literal_texts(&result.tokens)
        ),
    };
    assert_eq!(error.kind, FailureKind::Binding);
}
#[test]
fn pinned_fragment_matching_honors_definition_edition_and_expr_2021() {
    for (edition, fragment, expected) in [
        (Edition::E2021, "expr", "2"),
        (Edition::E2024, "expr", "1"),
        (Edition::E2024, "expr_2021", "2"),
    ] {
        let rules = DeclarativeMacro::new(definition(
            &format!("($value:{fragment}) => {{1}}; (const $value:block) => {{2}};"),
            edition,
        ))
        .unwrap();
        let args = input("const { 7 }");
        let result = rules.expand(&args, call_site(&args)).unwrap().unwrap();
        assert_eq!(literal_texts(&result.tokens), [expected]);
    }
}
#[test]
fn pinned_declarative_reports_definition_leftover_and_no_rule_errors_distinctly() {
    let rules =
        DeclarativeMacro::new(definition("($value:literal) => {$value};", Edition::E2024)).unwrap();
    let args = input("1 2");
    assert_eq!(
        rules
            .expand(&args, call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        FailureKind::LeftoverTokens
    );
    let args = input("name");
    assert_ne!(
        rules
            .expand(&args, call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        FailureKind::LeftoverTokens
    );
    let rules = DeclarativeMacro::new(definition("", Edition::E2024)).unwrap();
    assert_eq!(
        rules
            .expand(&args, call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        FailureKind::NoMatchingRule
    );
    let rules =
        DeclarativeMacro::new(definition("($value:literal) {$value};", Edition::E2024)).unwrap();
    assert_eq!(
        rules
            .expand(&args, call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        FailureKind::DefinitionSyntax
    );
}

#[test]
fn patched_repetition_checks_zero_lengths_and_preserves_ragged_nested_axes() {
    let rules = DeclarativeMacro::new(definition(
        "($($a:literal),*; $($b:literal),*) => {[$(($a,$b)),*]};",
        Edition::E2024,
    ))
    .unwrap();
    for source in ["1;", ";1", "1,2;3", "1;2,3"] {
        let args = input(source);
        let error = match rules.expand(&args, call_site(&args)).unwrap() {
            Err(e) => e,
            Ok(_) => panic!("unequal repetition cardinality accepted"),
        };
        assert_eq!(error.kind, FailureKind::Binding);
    }
    let empty = input(";");
    assert!(
        literal_texts(
            &rules
                .expand(&empty, call_site(&empty))
                .unwrap()
                .unwrap()
                .tokens
        )
        .is_empty()
    );
    let rules = DeclarativeMacro::new(definition(
        "($(($($a:literal),*; $($b:literal),*)),*) => { ($([$(( $a, $b )),*]),*) };",
        Edition::E2024,
    ))
    .unwrap();
    let args = input("(1,2;11,12),(3;13)");
    let result = rules.expand(&args, call_site(&args)).unwrap().unwrap();
    assert_eq!(
        literal_texts(&result.tokens),
        ["1", "11", "2", "12", "3", "13"]
    );
    let args = input("(1,2;11),(3;13)");
    assert_eq!(
        rules
            .expand(&args, call_site(&args))
            .unwrap()
            .err()
            .unwrap()
            .kind,
        FailureKind::Binding
    );
}
#[test]
fn syntax_only_fragment_matching_does_not_resolve_or_typecheck_cold_expressions() {
    let rules = DeclarativeMacro::new(definition("($value:expr) => {1};", Edition::E2024)).unwrap();
    let args = input("missing_function() + true");
    let result = rules.expand(&args, call_site(&args)).unwrap().unwrap();
    assert_eq!(literal_texts(&result.tokens), ["1"]);
    assert_eq!(result.matched_arm, 0);
}

#[test]
fn legal_repetition_above_old_backend_cutoff_preserves_every_binding() {
    let rules = DeclarativeMacro::new(definition(
        "($($value:literal),*) => {[$($value),*]};",
        Edition::E2024,
    ))
    .unwrap();
    let source = std::iter::repeat_n("7", 65_537)
        .collect::<Vec<_>>()
        .join(",");
    let args = input(&source);
    let result = rules.expand(&args, call_site(&args)).unwrap();
    let output = match result {
        Ok(output) => output,
        Err(error) => panic!("legal bounded repetition refused: {error}"),
    };
    let literals = literal_texts(&output.tokens);
    assert_eq!(literals.len(), 65_537);
    assert!(literals.iter().all(|value| value == "7"));
}

#[test]
fn repetition_without_a_driving_binding_refuses_before_repeating_output() {
    for source in ["($value:literal) => {[$($value),*]};", "() => {[$(7),*]};"] {
        let rules = DeclarativeMacro::new(definition(source, Edition::E2024)).unwrap();
        let args = input(if source.starts_with("()") { "" } else { "7" });
        let failure = rules
            .expand(&args, call_site(&args))
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(failure.kind, FailureKind::Binding);
    }
}

#[test]
fn token_preflight_refuses_before_materialization_and_preserves_exact_boundaries() {
    let rules = DeclarativeMacro::new(definition(
        "($($v:literal),*) => {[$($v),*]};",
        Edition::E2024,
    ))
    .unwrap();
    let args = input("7,2,11");
    // Invisible output root, bracket group, three literals and two commas.
    for (limit, accepted) in [(6, false), (7, true)] {
        let (result, stats) = rules
            .expand_with_token_limit(&args, call_site(&args), limit)
            .unwrap();
        assert!(stats.preflight_calls >= 1);
        if accepted {
            let output = result.unwrap();
            assert_eq!(literal_texts(&output.tokens), ["7", "2", "11"]);
            assert_eq!(stats.admitted_tokens, 7);
            assert_eq!(stats.materializations, 1);
        } else {
            assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
            assert_eq!(stats.materializations, 0);
        }
    }
    let empty = input("");
    let (result, stats) = rules
        .expand_with_token_limit(&empty, call_site(&empty), 2)
        .unwrap();
    assert!(literal_texts(&result.unwrap().tokens).is_empty());
    assert_eq!(stats.admitted_tokens, 2);
    assert_eq!(stats.materializations, 1);
    assert!(
        rules
            .expand_with_token_limit(&args, call_site(&args), 2_000_001)
            .is_err()
    );
}

#[test]
fn oversized_real_transcription_stops_in_count_only_mode() {
    // Compact input/definition generates over two million tokens by actual pinned repetition.
    let suffix = std::iter::repeat_n("0", 1000).collect::<Vec<_>>().join(" ");
    let rules = DeclarativeMacro::new(definition(
        &format!("($($v:literal),*) => {{$($v {suffix})*}};"),
        Edition::E2024,
    ))
    .unwrap();
    let args = input(&std::iter::repeat_n("7", 2000).collect::<Vec<_>>().join(","));
    let (result, stats) = rules
        .expand_with_token_limit(&args, call_site(&args), 2_000_000)
        .unwrap();
    assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
    assert_eq!(stats.materializations, 0);
    assert!(stats.max_preflight_tokens > 2_000_000);
    assert!(stats.max_preflight_tokens <= 2_001_001);
}

#[test]
fn token_preflight_counts_empty_ragged_groups_and_rollback_like_owned_output() {
    use lrr_expand_adapter::token_wire::TokenTree;
    for (definition_source, input_source) in [
        ("($($v:literal),*) => {[$( 0 0 $v 0 0 ),*]};", ""),
        ("($($v:literal),*) => {[$(($v)),*]};", "7,2,11"),
        (
            "($(($($a:literal),*; $($b:literal),*)),*) => {($([$(( $a, $b )),*]),*)};",
            "(1,2;11,12),(3;13)",
        ),
        ("($v:expr) => {$v};", "missing_function() + true"),
    ] {
        let rules = DeclarativeMacro::new(definition(definition_source, Edition::E2024)).unwrap();
        let args = input(input_source);
        let (result, stats) = rules
            .expand_with_token_limit(&args, call_site(&args), 2_000_000)
            .unwrap();
        let output = result.unwrap();
        let mut stack: Vec<_> = output.tokens.roots().iter().collect();
        let mut actual = 0;
        while let Some(token) = stack.pop() {
            actual += 1;
            if let TokenTree::Group { children, .. } = token {
                stack.extend(children);
            }
        }
        assert_eq!(stats.admitted_tokens, actual, "{definition_source}");
        let (exact, stats) = rules
            .expand_with_token_limit(&args, call_site(&args), actual)
            .unwrap();
        assert_eq!(
            exact
                .unwrap_or_else(|error| panic!("{definition_source}: {error}"))
                .tokens,
            output.tokens
        );
        assert_eq!(stats.materializations, 1);
        if actual > 1 {
            let (rejected, stats) = rules
                .expand_with_token_limit(&args, call_site(&args), actual - 1)
                .unwrap();
            assert_eq!(rejected.err().unwrap().kind, FailureKind::Limit);
            assert_eq!(stats.materializations, 0);
        }
    }
}

#[test]
fn token_limit_failure_does_not_retry_a_later_matching_arm() {
    let rules = DeclarativeMacro::new(definition(
        "($v:literal) => {[$v,$v,$v,$v]}; ($v:literal) => {1};",
        Edition::E2024,
    ))
    .unwrap();
    let args = input("7");
    let (result, stats) = rules
        .expand_with_token_limit(&args, call_site(&args), 3)
        .unwrap();
    assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
    assert_eq!(stats.preflight_calls, 1);
    assert_eq!(stats.materializations, 0);
}

#[test]
fn byte_preflight_matches_codec_metadata_and_refuses_before_materialization() {
    let cases = [
        ("($($v:literal),*) => {[$( 0 0 $v 0 0 ),*]};", ""),
        ("($($v:literal),*) => {[$($v),*]};", "7,2,11"),
        (
            "($(($($a:literal),*; $($b:literal),*)),*) => {($([$(( $a, $b )),*]),*)};",
            "(1,2;11,12),(3;13)",
        ),
        ("($v:expr) => {$v};", "missing_function() + true"),
        ("($v:tt) => {[$v,$v]};", "r###\"héllo\"###"),
        ("($v:ident) => {[$v,$v]};", "r#type"),
    ];
    for (definition_source, input_source) in cases {
        let rules = DeclarativeMacro::new(definition(definition_source, Edition::E2024)).unwrap();
        let args = input(input_source);
        let (result, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, 32 * 1024 * 1024)
            .unwrap();
        let output = result.unwrap();
        let bytes = output.tokens.encode().unwrap().len();
        assert_eq!(stats.admitted_bytes, bytes, "{definition_source}");
        let (exact, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, bytes)
            .unwrap();
        assert_eq!(exact.unwrap().tokens, output.tokens);
        assert_eq!(stats.materializations, 1);
        let (rejected, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, bytes - 1)
            .unwrap();
        assert_eq!(rejected.err().unwrap().kind, FailureKind::Limit);
        assert_eq!(stats.materializations, 0);
    }
}

#[test]
fn compact_input_with_oversized_wire_is_refused_in_count_only_mode() {
    let rules = DeclarativeMacro::new(definition(
        "($v:literal) => {[$v,$v]}; ($v:literal) => {1};",
        Edition::E2024,
    ))
    .unwrap();
    let args = input(&format!("r#\"{}\"#", "é".repeat(9 * 1024 * 1024)));
    let (result, stats) = rules
        .expand_with_limits(&args, call_site(&args), 2_000_000, 32 * 1024 * 1024)
        .unwrap();
    assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
    assert_eq!(stats.materializations, 0);
    assert_eq!(stats.preflight_calls, 1);
    assert!(stats.max_preflight_tokens < 10);
    assert!(stats.max_preflight_bytes > 32 * 1024 * 1024);
}

#[test]
fn byte_preflight_enforces_real_32mib_boundary_before_materialization() {
    const LIMIT: usize = 32 * 1024 * 1024;
    let rules = DeclarativeMacro::new(definition("($v:literal) => {$v};", Edition::E2024)).unwrap();
    let small = input("\"\"");
    let (result, stats) = rules
        .expand_with_limits(&small, call_site(&small), 2_000_000, LIMIT)
        .unwrap();
    assert_eq!(
        stats.admitted_bytes,
        result.unwrap().tokens.encoded_len().unwrap()
    );
    let overhead = stats.admitted_bytes - 2;
    for extra in [0, 1] {
        let args = input(&format!("\"{}\"", "a".repeat(LIMIT - overhead - 2 + extra)));
        let (result, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, LIMIT)
            .unwrap();
        if extra == 0 {
            let output = result.unwrap();
            assert_eq!(stats.admitted_bytes, LIMIT);
            assert_eq!(output.tokens.encoded_len().unwrap(), LIMIT);
            assert_eq!(output.tokens.encode().unwrap().len(), LIMIT);
            assert_eq!(stats.materializations, 1);
        } else {
            assert_eq!(result.err().unwrap().kind, FailureKind::Limit);
            assert_eq!(stats.materializations, 0);
            assert_eq!(stats.max_preflight_bytes, LIMIT + 1);
        }
    }
}

#[test]
fn admitted_replay_skips_empty_after_end_trials_and_trailing_separators() {
    use lrr_expand_adapter::token_wire::TokenTree;
    for source in ["", "7,2,11"] {
        let rules = DeclarativeMacro::new(definition(
            "($($v:literal),*) => {[$(0 0 0 $v),*]};",
            Edition::E2024,
        ))
        .unwrap();
        let args = input(source);
        let (result, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, 32 * 1024 * 1024)
            .unwrap();
        let output = result.unwrap();
        let mut stack: Vec<_> = output.tokens.roots().iter().collect();
        let mut leaves = 0;
        while let Some(token) = stack.pop() {
            if let TokenTree::Group { children, .. } = token {
                stack.extend(children);
            } else {
                leaves += 1;
            }
        }
        assert_eq!(
            stats.materialized_leaves, leaves,
            "discarded leaves materialized for {source:?}"
        );
    }
}

#[test]
fn admitted_nested_replay_preserves_zero_rows_and_bounds_temporary_groups() {
    let mut pattern = "$v:literal".to_owned();
    let mut template = "$v".to_owned();
    const DEPTH: usize = 80;
    for _ in 0..DEPTH {
        pattern = format!("$(({pattern}))*");
        template = format!("$({template})*");
    }
    let rules = DeclarativeMacro::new(definition(
        &format!("({pattern}) => {{{template}}};"),
        Edition::E2024,
    ))
    .unwrap();
    for source in [
        "".to_owned(),
        format!("{}7{}", "(".repeat(DEPTH), ")".repeat(DEPTH)),
    ] {
        let args = input(&source);
        let (result, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, 32 * 1024 * 1024)
            .unwrap();
        let output = result.unwrap();
        let values = literal_texts(&output.tokens);
        if source.is_empty() {
            assert!(values.is_empty());
            assert_eq!(stats.materialized_leaves, 0);
            assert_eq!(stats.materialized_raw_peak, 1);
        } else {
            assert_eq!(values, ["7"]);
            assert_eq!(stats.materialized_leaves, 1);
            assert_eq!(stats.materialized_raw_peak, DEPTH + 2);
        }
        assert!(stats.materialized_raw_peak <= stats.admitted_tokens + 256);
        assert_eq!(stats.admitted_bytes, output.tokens.encoded_len().unwrap());
    }
}

#[test]
fn stable_profile_rejects_nightly_metavariable_expression_on_definition_demand() {
    let rules = DeclarativeMacro::new(definition(
        "($($v:tt),*) => {${count($v)}};",
        Edition::E2024,
    ))
    .unwrap();
    let args = input("a,b");
    let (result, stats) = rules
        .expand_with_limits(&args, call_site(&args), 2_000_000, 32 * 1024 * 1024)
        .unwrap();
    assert_eq!(result.err().unwrap().kind, FailureKind::DefinitionSyntax);
    assert_eq!(stats.preflight_calls, 0);
    assert_eq!(stats.materializations, 0);
}

#[test]
fn stable_profile_keeps_dollar_literals_nested_unbound_vars_and_unsafe_body_syntax() {
    for rules_source in [
        "($v:tt) => {\"${count($v)} $$ unsafe\"};",
        "($v:tt) => {macro_rules! child {($bi:ident)=>{$bi};}};",
        "($v:tt) => {unsafe {missing_function()}};",
    ] {
        let rules = DeclarativeMacro::new(definition(rules_source, Edition::E2024)).unwrap();
        let args = input("7");
        assert!(rules.expand(&args, call_site(&args)).unwrap().is_ok());
    }
    for rules_source in [
        "($v:tt) => {$$};",
        "unsafe ($v:tt) => {$v};",
        "attr() ($v:tt) => {$v};",
    ] {
        let rules = DeclarativeMacro::new(definition(rules_source, Edition::E2024)).unwrap();
        let args = input("7");
        let (result, stats) = rules
            .expand_with_limits(&args, call_site(&args), 2_000_000, 32 * 1024 * 1024)
            .unwrap();
        assert_eq!(result.err().unwrap().kind, FailureKind::DefinitionSyntax);
        assert_eq!(stats.preflight_calls, 0);
    }
}
