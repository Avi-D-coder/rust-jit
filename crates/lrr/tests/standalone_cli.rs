use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(source: &str) -> std::process::Output {
    run_in_edition(source, 2024)
}
fn run_in_edition(source: &str, edition: u16) -> std::process::Output {
    let root = std::env::temp_dir().join(format!(
        "lrr-scalar-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("main.rs");
    fs::write(&file, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("run")
        .arg(&file)
        .arg("--edition")
        .arg(edition.to_string())
        .output()
        .unwrap();
    fs::remove_dir_all(root).unwrap();
    output
}
#[test]
fn rustc_original_empty_main_does_not_force_unused_duplicate_field_schema() {
    let output = run(include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/structs/struct-fields-decl-dupe.rs"
    ));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert!(output.stderr.is_empty());
    // LRR demand semantics contrast with rustc's whole-crate E0124 expectation;
    // this assertion is not native compiletest diagnostic parity.
}
#[test]
fn standalone_cli_reports_actual_lrir_execution_and_fails_selected_unsupported() {
    let output = run("fn main(){let x:i64=40_i64;x;} fn cold<T: Missing>(){undefined();}");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "completed");
    assert_eq!(result["value"]["kind"], "unit");
    assert_eq!(result["opcode_counters"]["operations_completed"], 12);
    assert_eq!(result["installed_recipes"], 8);
    assert!(result.get("type_id").is_none());
    assert!(result.get("payload").is_none());
    let fault = run("fn main(){missing();}");
    assert!(!fault.status.success());
    assert!(fault.stdout.is_empty());
    assert!(String::from_utf8_lossy(&fault.stderr).contains("LRR-E-NAME-MISSING"));
}

#[test]
fn standalone_cli_cannot_ignore_a_selected_attribute_macro() {
    let output = run("#[not_prepared] fn main(){let x:i64=40_i64;x;}");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
}

#[test]
fn standalone_cli_fails_pending_statement_attributes_and_entry_parameters() {
    for (source, expected) in [
        (
            "fn main(){#[not_prepared] let x:i64=40_i64;x;}",
            "LRR-E-CATALOG-INCOMPLETE",
        ),
        ("fn main(x:i64){40_i64;}", "LRR-E-UNSUPPORTED"),
    ] {
        let output = run(source);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    }
}

#[test]
fn standalone_cli_reports_source_arithmetic_guest_panic_with_cleanup_and_nonzero_status() {
    let output = run("fn main(){let x:i8=127_i8+1_i8;missing();}");
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "guest_panic");
    assert_eq!(report["panic"]["kind"], "overflow");
    assert_eq!(report["opcode_counters"]["panic_scopes_cleaned"], 2);
    assert!(output.stderr.is_empty());
    // This explicitly typed mismatch checks the storage contract. General
    // later-use inference for an unannotated operator binding remains pending.
    let fault = run("fn main(){let x:i32=3+4;let later:i64=x;}");
    assert_eq!(fault.status.code(), Some(2));
    assert!(fault.stdout.is_empty());
    assert!(String::from_utf8_lossy(&fault.stderr).contains("LRR-E-OWNERSHIP"));
}

#[test]
fn standalone_cli_executes_float_and_escaped_char_source_without_raw_value_exports() {
    let output = run(r"fn main(){let c:char='\u{1F980}';let f:f32=1_f32+2_f32;c;f;}");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][10], 1);
    assert!(report.get("payload").is_none());
    for edition in [2015, 2018, 2021, 2024] {
        let output = run_in_edition("fn main(){let x=1.0+2.0;let later:f64=x;}", edition);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "completed");
        assert_eq!(report["opcode_counters"]["started_by_opcode"][10], 1);
        assert_eq!(report["live_edition"], edition);
        assert!(report.get("payload").is_none());
    }
}

#[test]
fn standalone_cli_executes_nested_source_calls_with_actual_vm_seal_counts() {
    let output = run(
        "fn main()->i64{outer()+1i64}fn outer()->i64{leaf()+1i64}fn leaf<U:Missing>()->i64{40i64}fn cold<T:Absent>(x:Undefined){missing();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["value"]["kind"], "signed");
    assert_eq!(report["value"]["value"], "42");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 2);
    assert_eq!(report["opcode_counters"]["return_scopes_cleaned"], 3);
}

#[test]
fn standalone_cli_executes_ordered_arguments_and_parameter_frames() {
    let output = run(
        "fn main()->i64{add(leaf(20),leaf(21))}fn add(a:i64,b:i64)->i64{a+b}fn leaf(x:i64)->i64{x+1}fn cold<T:Missing>(){absent();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["value"]["value"], "43");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 3);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][21], 4);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][23], 4);
}

#[test]
fn standalone_cli_uses_generic_result_context_without_compiling_live_source() {
    let output = run(
        "fn main()->i64{identity(42)}fn identity<T:Missing,U:Other>(x:T)->T{x}fn cold<T:Undefined>(){missing();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["value"]["value"], "42");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 1);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][21], 1);
}

#[test]
fn standalone_cli_executes_explicit_type_arguments_with_unused_bounds() {
    let output = run(
        "fn main()->i64{identity::<i64>(42)}fn identity<'a,T:Missing,U:Other>(x:T)->T{x}fn cold<T:Undefined>(){missing();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["value"]["value"], "42");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 1);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][21], 1);
}

#[test]
fn standalone_cli_checks_generic_local_declaration_and_initialization() {
    let output = run(
        "fn main()->i64{copy(42)}fn copy<T:Missing>(x:T)->T{let mut y:T;y=x;y}fn cold<T:Undefined>(){missing();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["value"]["value"], "42");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][14], 1);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][16], 1);
}

#[test]
fn standalone_cli_executes_qualified_generic_calls_after_local_declarations() {
    let output = run(
        "mod values{pub fn identity<T:Missing>(x:T)->T{x}}use values as alias;fn main()->i64{fn local(){broken();}alias::identity::<i64>(42)}fn cold<T:Undefined>(){missing();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["value"]["value"], "42");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][24], 1);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 1);
}

#[test]
fn standalone_cli_executes_builtin_assertion_predicate_and_keeps_failure_arguments_cold() {
    let output = run(
        "fn predicate()->bool{true} fn main(){assert!(predicate(), \"{}\", missing());} fn cold(){broken();}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["value"]["kind"], "unit");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 1);
    assert_eq!(report["opcode_counters"]["started_by_opcode"][26], 0);
}
#[test]
fn standalone_cli_reports_real_assertion_and_qualified_panic_messages() {
    for (source, message, kind) in [
        (
            "fn main(){assert!(false, \"failure\");}",
            "failure",
            "assertion",
        ),
        (
            "fn main(){std::panic!(\"qualified\");}",
            "qualified",
            "explicit",
        ),
    ] {
        let output = run(source);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "guest_panic");
        assert_eq!(report["panic"]["message"], message);
        assert_eq!(report["panic"]["kind"], kind);
        assert!(
            report["opcode_counters"]["panic_scopes_cleaned"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
}

#[test]
fn edition2024_raw_gen_declaration_and_call_execute_through_guest_dispatch() {
    let output = run("fn r#gen()->bool{true}fn main(){assert!(r#gen());}");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["opcode_counters"]["started_by_opcode"][19], 1);
}
#[test]
fn edition2024_bare_gen_declaration_reports_parser_edition_before_name_fallback() {
    let output = run("fn gen()->bool{true}fn main(){gen();}");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("LRR-E-PARSE-BLOCKS-ENTRY"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("2024"), "{diagnostic}");
    assert!(!diagnostic.contains("LRR-E-NAME-MISSING"));
    assert!(!diagnostic.contains("LRR-E-MACRO-SCOPE"));
}

#[test]
fn standalone_cli_reports_current_live_edition_and_profile_on_completion_and_guest_panic() {
    for source in ["fn main(){}", "fn main(){panic!(\"profile panic\");}"] {
        for edition in [2015, 2018, 2021, 2024] {
            let output = run_in_edition(source, edition);
            assert_eq!(
                output.status.code(),
                Some(if source.contains("panic!") { 1 } else { 0 }),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(report["live_edition"], edition);
            assert_eq!(report["semantics_profile"], "lrr-2024-v2");
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for edition in ["2015", "2018", "2021", "2024"] {
        assert!(help.contains(edition));
    }
    assert!(help.contains("--edition"));
}

// Unchanged official source supplies the entire language program. The newline-only
// variant is an explicit LRR display-freshness adaptation outside the licensed copy.
#[test]
fn rustc_missing_macro_argument_renders_owned_invocation_and_definition_locations() {
    let stock = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
    );
    for (prefix, invocation_line, definition_line) in [("", 9, 2), ("\n\n", 11, 4)] {
        let source = format!("{prefix}{stock}");
        let output = run(&source);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty(), "failure emitted a success report");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("LRR-E-MACRO-EXPANSION"), "{diagnostic}");
        assert!(diagnostic.contains("expected Expr"), "{diagnostic}");
        assert!(
            diagnostic.contains(&format!("main.rs:{invocation_line}:5")),
            "missing current invocation location: {diagnostic}"
        );
        assert!(
            diagnostic.contains(&format!("main.rs:{definition_line}:1")),
            "missing actual definition keyword location: {diagnostic}"
        );
        assert!(
            diagnostic.contains("one_arg_macro"),
            "missing selected definition name: {diagnostic}"
        );
        assert!(
            diagnostic.contains("|     one_arg_macro!();"),
            "missing captured invocation excerpt: {diagnostic}"
        );
        assert!(
            diagnostic.contains("| macro_rules! one_arg_macro {"),
            "missing captured definition excerpt: {diagnostic}"
        );
    }
}

// Whole unchanged upstream programs are run in test-discovery mode too. The
// newline adapter stays outside the licensed stock directory.
#[test]
fn rustc_test_discovery_failures_render_current_source_macro_chain() {
    for (stock, invocation_line, definition_line, nested) in [
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
            ),
            9,
            2,
            false,
        ),
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/best-failure.rs"
            ),
            9,
            1,
            true,
        ),
    ] {
        let root = std::env::temp_dir().join(format!(
            "lrr-test-origin-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("main.rs");
        let artifacts = root.join("diagnostics");
        for prefix in ["", "\n\n"] {
            fs::write(&file, format!("{prefix}{stock}")).unwrap();
            let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
                .args(["test", "--diagnostic-dir"])
                .arg(&artifacts)
                .arg(&file)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(diagnostic.contains("LRR-E-MACRO-EXPANSION"), "{diagnostic}");
            let delta = prefix.lines().count();
            assert!(
                diagnostic.contains(&format!(
                    "main.rs:{}:{}",
                    invocation_line + delta,
                    if nested { 1 } else { 5 }
                )),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains(&format!("main.rs:{}:1", definition_line + delta)),
                "{diagnostic}"
            );
            if nested {
                assert!(
                    diagnostic.contains(&format!("main.rs:{}:", 4 + delta)),
                    "{diagnostic}"
                );
            }
            assert!(
                !artifacts.exists(),
                "ordinary discovery failure wrote an internal bundle"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}

// The stock program deliberately emits a nested failing invocation from a macro
// template. Only the display-freshness newline prefix is an LRR adaptation.
#[test]
fn rustc_nested_macro_failure_traces_outer_invocation_and_selected_definition() {
    let stock =
        include_str!("../../../fixtures/upstream/rustc-checkout/tests/ui/macros/best-failure.rs");
    for (prefix, outer_line, definition_line, failure_line) in [("", 9, 1, 4), ("\n\n", 11, 3, 6)] {
        let output = run(&format!("{prefix}{stock}"));
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("LRR-E-MACRO-EXPANSION"), "{diagnostic}");
        assert!(diagnostic.contains("number"), "{diagnostic}");
        assert!(
            diagnostic.contains(&format!("main.rs:{outer_line}:1")),
            "missing actual outer invocation: {diagnostic}"
        );
        assert!(
            diagnostic.contains(&format!("main.rs:{definition_line}:1")),
            "missing selected definition: {diagnostic}"
        );
        assert!(
            diagnostic.contains(&format!("main.rs:{failure_line}:")),
            "missing captured matcher failure: {diagnostic}"
        );
    }
}

#[test]
fn rustc_lifetime_and_reserved_identifier_failures_keep_current_source_locations() {
    let cases = [
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-non-lifetime.rs"
            ),
            "m",
            "expected lifetime",
            6,
            5,
            3,
        ),
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-fragment-ident-underscore-error.rs"
            ),
            "identity",
            "expected ident",
            11,
            9,
            4,
        ),
    ];
    for (stock, name, failure, invocation_line, invocation_column, definition_line) in cases {
        for (prefix, shift) in [("", 0), ("\n\n", 2)] {
            let output = run(&format!("{prefix}{stock}"));
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(diagnostic.contains("LRR-E-MACRO-EXPANSION"), "{diagnostic}");
            assert!(diagnostic.contains(failure), "{diagnostic}");
            assert!(
                diagnostic.contains(&format!(
                    "main.rs:{}:{invocation_column}",
                    invocation_line + shift
                )),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains(&format!(
                    "main.rs:{}:1: definition of `{name}`",
                    definition_line + shift
                )),
                "{diagnostic}"
            );
        }
    }
}

// These entire unchanged sources require later expression/borrow integration.
// This assertion covers M016's pending-operation diagnostic only; native type
// and borrow expectations stay unfulfilled and cannot count as fixture passes.
#[test]
fn rustc_reached_pending_expression_macros_keep_selected_source_locations() {
    let cases = [
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/issue-29084.rs"
            ),
            "foo",
            13,
            3,
        ),
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/span-covering-argument-1.rs"
            ),
            "bad",
            12,
            1,
        ),
    ];
    for (stock, name, invocation_line, definition_line) in cases {
        for (prefix, shift) in [("", 0), ("\n\n", 2)] {
            let output = run(&format!("{prefix}{stock}"));
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(
                diagnostic.contains("LRR-E-CATALOG-INCOMPLETE"),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains("requires source expansion integration"),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains(&format!("main.rs:{}:5", invocation_line + shift)),
                "missing selected invocation: {diagnostic}"
            );
            assert!(
                diagnostic.contains(&format!(
                    "main.rs:{}:1: definition of `{name}`",
                    definition_line + shift
                )),
                "missing selected definition: {diagnostic}"
            );
        }
    }
}

// Pending is preserved: this does not promote an unsealed namespace to absence
// or claim full rustc diagnostic recovery after the first blocked producer.
#[test]
fn rustc_unresolved_fixedpoint_macro_names_actual_pending_source_invocation() {
    let stock = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro_undefined.rs"
    );
    // Vary captured content/path identities: digest-order selection previously
    // chose the later kl!() producer intermittently in the broader CLI suite.
    for shift in 0..8 {
        let prefix = "\n".repeat(shift);
        let line = 11 + shift;
        let output = run(&format!("{prefix}{stock}"));
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("LRR-E-CATALOG-INCOMPLETE"),
            "{diagnostic}"
        );
        assert!(
            diagnostic.contains("fixed point retains unresolved"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("blocked macro `k`"), "{diagnostic}");
        assert!(
            diagnostic.contains(&format!("main.rs:{line}:5")),
            "{diagnostic}"
        );
        assert!(
            !diagnostic.contains("definition of `kl`"),
            "unrelated exported definition attributed to blocked invocation: {diagnostic}"
        );
    }
}

#[test]
fn rustc_emitted_macro_failure_has_local_explanation_without_source_access() {
    let stock = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
    );
    let failed = run(stock);
    assert_eq!(failed.status.code(), Some(2));
    let diagnostic = String::from_utf8(failed.stderr).unwrap();
    let code = diagnostic.split_once(':').unwrap().0;
    let explanation = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .args(["explain", code])
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(
        explanation.status.success(),
        "{}",
        String::from_utf8_lossy(&explanation.stderr)
    );
    assert!(explanation.stderr.is_empty());
    let text = String::from_utf8(explanation.stdout).unwrap();
    assert!(text.starts_with(code));
    for field in [
        "Invariant:",
        "Phase:",
        "Effects may already have occurred:",
        "Cleanup runs:",
        "Example:",
        "Next step:",
    ] {
        assert!(text.contains(field), "{text}");
    }
    assert!(text.contains("macro-invocation-span-error-7970.rs"));
    assert!(!text.contains("opcode_counters"), "explanation ran a guest");
    let unknown = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .args(["explain", "LRR-E-UNKNOWN-EXPLANATION-CONTROL"])
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown diagnostic code"));
}

#[test]
fn rustc_macro_diagnostics_do_not_write_bundles_for_ordinary_source_failures() {
    let root = std::env::temp_dir().join(format!(
        "lrr-stock-bundle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("main.rs");
    fs::write(&source, include_str!("../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs")).unwrap();
    let directory = root.join("diagnostics");
    let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("run")
        .arg(&source)
        .arg("--diagnostic-dir")
        .arg(&directory)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-MACRO-EXPANSION"));
    assert!(
        !directory.exists(),
        "source diagnostic created an internal artifact"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rustc_original_scalar_assertion_programs_execute_every_original_assertion() {
    // Unchanged upstream bodies; the owning live profile is explicitly 2024.
    // Native 2015/2024 references are separate authorized preparation evidence.
    // No expected arithmetic result is supplied by this harness.
    for original in [
        include_str!(
            "../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/i32-sub.rs"
        ),
        include_str!(
            "../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/arith-unsigned.rs"
        ),
        include_str!(
            "../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/floatlits.rs"
        ),
        include_str!(
            "../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-radix.rs"
        ),
    ] {
        // These pinned originals have no quoted/commented assertion invocations.
        let assertions =
            original.matches("assert!(").count() + original.matches("assert_eq!(").count();
        assert!(assertions > 0);
        let output = run(original);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "completed");
        assert_eq!(report["live_edition"], 2024);
        assert_eq!(report["semantics_profile"], "lrr-2024-v2");
        assert_eq!(report["value"]["kind"], "unit");
        assert_eq!(
            report["opcode_counters"]["started_by_opcode"][26].as_u64(),
            Some(u64::try_from(assertions).unwrap())
        );
        assert_eq!(
            report["opcode_counters"]["operations_started"],
            report["opcode_counters"]["operations_completed"]
        );
        assert_eq!(report["opcode_counters"]["panic_scopes_cleaned"], 0);
    }
}

#[test]
fn rustc_original_isize_cast_uses_rust_source_fallback_and_original_assertions() {
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/isize-base.rs"
    );
    // E-NUM-001: the literal-only cast source falls back independently to i32;
    // its written isize target still controls the executed primitive conversion.
    // Execute the complete unchanged upstream source and its original assertions.
    let assertions = original.matches("assert!(").count() + original.matches("assert_eq!(").count();
    assert!(assertions > 0);
    let output = run(original);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "lrr/v2/development-run");
    assert_eq!(report["status"], "completed");
    assert_eq!(report["live_edition"], 2024);
    assert_eq!(report["semantics_profile"], "lrr-2024-v2");
    assert_eq!(report["value"]["kind"], "unit");
    assert_eq!(
        report["opcode_counters"]["started_by_opcode"][26].as_u64(),
        Some(u64::try_from(assertions).unwrap())
    );
    assert_eq!(
        report["opcode_counters"]["operations_started"],
        report["opcode_counters"]["operations_completed"]
    );
    assert_eq!(report["opcode_counters"]["panic_scopes_cleaned"], 0);
}

#[test]
fn unchanged_stock_generic_program_cli_retains_each_official_root_edition() {
    let stock = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-fn-infer.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
            .arg("run")
            .arg(&stock)
            .arg("--edition")
            .arg(edition.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "edition {edition}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["live_edition"], edition);
        assert_eq!(report["semantics_profile"], "lrr-2024-v2");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("run")
        .arg(&stock)
        .arg("--edition")
        .arg("2099")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-LIVE-EDITION"));
}
