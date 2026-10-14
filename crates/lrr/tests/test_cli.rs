use std::{
    fs,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn generated_test_catalog_lists_final_metadata_before_selecting_unsupported_body() {
    let source = r#"
macro_rules! make_tests { () => {
    #[test] fn normal() {}
    #[test] #[ignore = "later"] fn ignored() { missing(); }
    #[test] #[should_panic(expected = "needle")] fn paniccase() { panic!("needle"); }
    #[test] async fn later() {}
}; }
make_tests!();
"#;
    let output = test(source, &["--include-ignored", "--list"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        json(&output)["listed"],
        serde_json::json!(["ignored", "later", "normal", "paniccase"])
    );
    let output = test(source, &["later", "--exact"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json(&output)["results"][0]["diagnostic"]["code"],
        "LRR-E-UNSUPPORTED"
    );
    let output = test(source, &["normal", "--exact"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output)["results"][0]["status"], "passed");
}

#[test]
fn generated_test_catalog_keeps_required_expansion_failure_ahead_of_filtering() {
    let source = "macro_rules! outer { () => { unknown_test_factory!(); #[test] fn known() {} }; } outer!();";
    for args in [
        vec!["known", "--exact", "--list"],
        vec!["kno", "--list"],
        vec!["absent", "--exact", "--list"],
    ] {
        let output = test(source, &args);
        assert_eq!(output.status.code(), Some(2));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("LRR-E-CATALOG-INCOMPLETE"));
        assert!(!error.contains("LRR-E-TEST-NO-MATCH"));
        assert!(
            output.stdout.is_empty(),
            "failed discovery published a test summary"
        );
    }
}

#[test]
fn rustc_original_test_listing_retains_sorted_names_and_ignore_metadata() {
    let source = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/tests-listing-format-default.rs"
    );
    let output = test(source, &["--include-ignored", "--list"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        json(&output)["listed"],
        serde_json::json!(["a_test", "m_test", "z_test"])
    );
    let output = test(source, &["--ignored", "--list"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["listed"], serde_json::json!(["z_test"]));
}

fn test(source: &str, args: &[&str]) -> Output {
    let directory = std::env::temp_dir().join(format!(
        "lrr-test-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&directory).unwrap();
    let file = directory.join("tests.rs");
    fs::write(&file, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("test")
        .arg(file)
        .args(args)
        .output()
        .unwrap();
    fs::remove_dir_all(directory).unwrap();
    output
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)))
}
#[test]
fn selected_test_runs_without_preparing_an_unselected_broken_body() {
    let output = test(
        "#[test]fn chosen(){}#[test]fn broken(){unknown();}",
        &["chosen", "--exact"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = json(&output);
    assert_eq!(report["selected"], 1);
    assert_eq!(report["filtered_out"], 1);
    assert_eq!(report["results"][0]["status"], "passed");
}
#[test]
fn test_listing_exact_substring_and_no_match_follow_the_cli_contract() {
    let source = "#[test]fn first(){}mod nested{#[test]fn first_more(){unknown();}}#[test]#[ignore]fn ignored(){unknown();}";
    let output = test(source, &["first", "--list"]);
    assert!(output.status.success());
    assert_eq!(
        json(&output)["listed"],
        serde_json::json!(["first", "nested::first_more"])
    );
    let output = test(source, &["first", "--exact", "--list"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["listed"], serde_json::json!(["first"]));
    let output = test(source, &["absent", "--exact"]);
    assert_eq!(output.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-TEST-NO-MATCH"));
    let output = test(source, &["--ignored", "--list"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["listed"], serde_json::json!(["ignored"]));
    let output = test(source, &["--ignored", "--include-ignored"]);
    assert_eq!(output.status.code(), Some(2));
}
#[test]
fn expected_guest_panic_accepts_numeric_panic_but_never_an_unsupported_fault() {
    let source = "#[test]#[should_panic(expected=\"overflow\")]fn overflow(){let x=127i8;x+1i8;}#[test]#[should_panic]fn unsupported(){async {};}#[test]#[should_panic(expected=\"other\")]fn mismatch(){let x=127i8;x+1i8;}";
    let output = test(source, &["overflow", "--exact"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output)["results"][0]["status"], "passed");
    let output = test(source, &["unsupported", "--exact"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json(&output)["results"][0]["diagnostic"]["code"],
        "LRR-E-UNSUPPORTED"
    );
    let output = test(source, &["mismatch", "--exact"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json(&output)["results"][0]["status"], "failed");
}
#[test]
fn malformed_control_attributes_fail_only_when_selected_and_async_tests_stay_visible() {
    let source = "#[test]fn good(){}#[test]#[should_panic(unexpected=1)]fn malformed(){}#[test]async fn asynchronous(){}";
    let output = test(source, &["good", "--exact"]);
    assert!(output.status.success());
    let output = test(source, &["malformed", "--exact"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json(&output)["results"][0]["diagnostic"]["code"],
        "LRR-E-TEST-DISCOVERY"
    );
    let output = test(source, &["asynchronous", "--exact", "--list"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["listed"], serde_json::json!(["asynchronous"]));
    let output = test(source, &["asynchronous", "--exact"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json(&output)["results"][0]["diagnostic"]["code"],
        "LRR-E-UNSUPPORTED"
    );
}
#[test]
fn active_catalog_producers_fail_discovery_even_when_filter_matches_nothing() {
    let output = test("#[make_tests]fn producer(){}", &["absent", "--exact"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
}
#[test]
fn test_cfg_and_multiple_selected_cases_execute_with_fresh_guest_heaps() {
    let output = test(
        "#[cfg(test)]#[test]fn one(){let mut x=0i64;x=x+1;}#[cfg(test)]#[test]fn two(){let mut x=0i64;x=x+2;}#[test]#[ignore=\"later\"]fn ignored(){unknown();}",
        &[],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = json(&output);
    assert_eq!(report["selected"], 2);
    assert!(
        report["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|result| result["status"] == "passed")
    );
}

#[test]
fn cli_selected_builtin_assertion_passes_with_broken_unselected_body() {
    let output = test(
        "#[test]fn chosen(){assert!(true, \"{}\", missing());} #[test]fn broken(){unknown();}",
        &["chosen", "--exact"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = json(&output);
    assert_eq!(report["selected"], 1);
    assert_eq!(report["filtered_out"], 1);
    assert_eq!(report["results"][0]["status"], "passed");
}
#[test]
fn cli_compiler_macro_prelude_and_std_core_qualified_exports_produce_guest_panics() {
    let source = "#[test]#[should_panic(expected=\"prelude\")]fn one(){panic!(\"prelude\");} #[test]#[should_panic(expected=\"std\")]fn two(){std::panic!(\"std\");} #[test]#[should_panic(expected=\"core\")]fn three(){core::panic!(\"core\");}";
    let output = test(source, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = json(&output);
    assert_eq!(report["selected"], 3);
    for result in report["results"].as_array().unwrap() {
        assert_eq!(result["status"], "passed");
    }
}
#[test]
fn cli_std_macro_alias_preserves_identity_and_source_predicate_calls() {
    let source = "use std::assert as check; fn predicate()->bool{true} #[test]fn chosen(){check!(predicate(), \"{}\", missing());}";
    let output = test(source, &["chosen", "--exact"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output)["results"][0]["status"], "passed");
}
#[test]
fn cli_user_macro_shadowing_and_qualified_absence_cannot_satisfy_expected_panic() {
    let source = "macro_rules! panic {($($t:tt)*)=>{};} #[test]#[should_panic]fn chosen(){panic!(user => grammar);}";
    let output = test(source, &["chosen", "--exact"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json(&output)["results"][0]["diagnostic"]["code"],
        "LRR-E-CATALOG-INCOMPLETE"
    );
    for source in [
        "#[test]#[should_panic]fn chosen(){self::panic!(\"not prelude\");}",
        "#[test]#[should_panic]fn chosen(){crate::panic!(\"not prelude\");}",
    ] {
        // Section 50: an unresolved producer prevents catalog publication before entry.
        let output = test(source, &["chosen", "--exact"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
    }
}
#[test]
fn cli_no_std_keeps_core_macros_and_excludes_automatic_std_routes() {
    let output = test(
        "#![no_std] #[test]#[should_panic(expected=\"core\")]fn chosen(){core::panic!(\"core\");}",
        &["chosen", "--exact"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output)["results"][0]["status"], "passed");
    let source =
        "#![no_std] #[test]fn chosen(){} #[test]#[should_panic]fn absent(){std::panic!(\"std\");}";
    for name in ["chosen", "absent"] {
        let output = test(source, &[name, "--exact"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
    }
}
#[test]
fn cli_cfg_aware_no_implicit_prelude_disables_macro_and_automatic_extern_lookup() {
    for source in [
        "#![no_implicit_prelude] #[test]#[should_panic]fn chosen(){assert_eq!(1,1);}",
        "#![cfg_attr(test, no_implicit_prelude)] #[test]#[should_panic]fn chosen(){std::panic!(\"disabled\");}",
        "#[no_implicit_prelude]mod nested{#[test]#[should_panic]fn chosen(){assert_eq!(1,1);}}",
        "mod nested{#![no_implicit_prelude] #[test]#[should_panic]fn chosen(){std::panic!(\"disabled\");}}",
    ] {
        let output = test(source, &["chosen"]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
    }
    let output = test(
        "#![cfg_attr(any(), no_implicit_prelude)]mod nested{#[test]fn chosen(){assert!(true);}}",
        &["chosen"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output)["results"][0]["status"], "passed");
}
#[test]
fn cli_macro_listing_refuses_argument_syntax_without_a_certified_no_item_summary() {
    // Section 50.3 requires a transitive argument scan. Invalid grammar cannot
    // produce that certificate, so both listing and entry fail during discovery.
    let source = "#[test]fn chosen(){assert!(not -> expression);} #[test]fn broken(){unknown();}";
    for args in [
        &["chosen", "--exact", "--list"][..],
        &["chosen", "--exact"][..],
    ] {
        let output = test(source, args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-RUNTIME-MACRO-ARGUMENTS"));
    }
}
#[test]
fn cli_disabled_scope_import_cannot_reintroduce_automatic_std_macro_identity() {
    let source = "#[no_implicit_prelude] mod closed{use std::panic as stop; #[test]#[should_panic]fn chosen(){stop!(\"disabled\");}} #[test]fn outside(){assert!(true);}";
    for args in [&["chosen"][..], &["outside", "--exact"][..]] {
        let output = test(source, args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(
            "LRR-E-CATALOG-INCOMPLETE: fixed point retains unresolved import or macro producers"
        ));
    }
    let inactive = "#[cfg(any())] #[no_implicit_prelude] mod closed{use std::panic as stop; #[test]fn chosen(){stop!(\"disabled\");}} #[test]fn outside(){assert!(true);}";
    let output = test(inactive, &["outside", "--exact"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output)["results"][0]["status"], "passed");
}

#[test]
fn cli_sequential_no_item_intrinsics_release_later_macro_lookup_without_expansion() {
    for source in [
        "#[test]#[should_panic(expected=\"later\")]fn chosen(){assert!(true); panic!(\"later\");}",
        "fn predicate()->bool{true} #[test]#[should_panic(expected=\"last\")]fn chosen(){assert!(predicate(), \"{}\", missing()); assert!(true); core::panic!(\"last\");}",
        "use std::assert as check; #[test]#[should_panic(expected=\"last\")]fn chosen(){check!(true); std::panic!(\"last\");}",
    ] {
        let output = test(source, &["chosen", "--exact"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(json(&output)["results"][0]["status"], "passed");
    }
}
#[test]
fn cli_no_item_summary_does_not_retire_nested_macro_or_declaration_argument_producers() {
    for source in [
        "#[test]#[should_panic]fn chosen(){assert!(true, \"{}\", unknown!()); panic!(\"later\");}",
        "#[test]#[should_panic]fn chosen(){assert!(true, \"{}\", {fn declaration(){} 1}); panic!(\"later\");}",
        "#[test]#[should_panic]fn chosen(){assert!(true, \"{}\", {impl Missing for Absent{} 1}); panic!(\"later\");}",
    ] {
        let output = test(source, &["chosen", "--exact"]);
        if source.contains("unknown!()") {
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
            continue;
        }
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = json(&output);
        assert_eq!(report["results"][0]["status"], "failed");
        assert_eq!(
            report["results"][0]["diagnostic"]["code"],
            "LRR-E-CATALOG-INCOMPLETE"
        );
    }
}

#[test]
fn cli_assertion_binding_predicate_reads_actual_local_shadow_and_parameter() {
    for source in [
        "#[test]fn chosen(){let value=true; assert!(value, \"{}\", missing());}",
        "fn verify(value:bool){assert!(value);} #[test]fn chosen(){verify(true);}",
        "#[test]fn chosen(){let value=false; {let value=true; assert!(value);} assert!(true);}",
        "#[test]fn chosen(){let mut value=true; let mut n=0i32; while n<3i32{assert!(value); n=n+1i32;} value=false;}",
    ] {
        let output = test(source, &["chosen", "--exact"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(json(&output)["results"][0]["status"], "passed");
    }
}
#[test]
fn cli_assertion_binding_failure_preserves_default_name_and_explicit_message() {
    for source in [
        "#[test]#[should_panic(expected=\"assertion failed: value\")]fn chosen(){let value=false; assert!(value);}",
        "fn verify(value:bool){assert!(value, \"parameter failed\");} #[test]#[should_panic(expected=\"parameter failed\")]fn chosen(){verify(false);}",
        "#[test]#[should_panic(expected=\"assertion failed: r#type\")]fn chosen(){let r#type=false; assert!(r#type);}",
        "#[test]#[should_panic(expected=\"assertion failed: value\")]fn chosen(){let mut value=true; assert!(value); value=false; assert!(value);}",
    ] {
        let output = test(source, &["chosen", "--exact"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(json(&output)["results"][0]["status"], "passed");
    }
}
#[test]
fn cli_assertion_binding_type_absence_and_qualification_faults_cannot_satisfy_panic() {
    for source in [
        "#[test]#[should_panic]fn chosen(){let value=1i32; assert!(value);}",
        "#[test]#[should_panic]fn chosen(){assert!(value);}",
        "#[test]#[should_panic]fn chosen(){let value=true; assert!(self::value);}",
    ] {
        let output = test(source, &["chosen", "--exact"]);
        assert_eq!(output.status.code(), Some(1));
        let report = json(&output);
        assert_eq!(report["results"][0]["status"], "failed");
        assert!(
            report["results"][0]["diagnostic"]["code"]
                .as_str()
                .is_some()
        );
    }
}

#[test]
fn cli_test_listing_and_execution_report_current_live_edition_and_profile() {
    for args in [
        &["chosen", "--exact"][..],
        &["chosen", "--exact", "--list"][..],
    ] {
        for edition in ["2015", "2018", "2021", "2024"] {
            let mut selected = args.to_vec();
            selected.extend(["--edition", edition]);
            let output = test("#[test]fn chosen(){}", &selected);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report = json(&output);
            assert_eq!(report["live_edition"], edition.parse::<u16>().unwrap());
            assert_eq!(report["semantics_profile"], "lrr-2024-v2");
        }
    }
}

#[test]
fn cli_compiler_global_panic_and_assert_match_native_with_disabled_preludes() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(
        version_text.contains("release: 1.99.0")
            && version_text.contains("host: aarch64-apple-darwin")
    );
    for source in [
        "#![no_implicit_prelude]#[test]#[should_panic(expected=\"global\")]fn chosen(){assert!(true);panic!(\"global\");}",
        "#[no_implicit_prelude]mod nested{#[test]#[should_panic(expected=\"global\")]fn chosen(){assert!(true);panic!(\"global\");}}",
        "mod nested{#![no_implicit_prelude]#[test]#[should_panic(expected=\"global\")]fn chosen(){assert!(true);panic!(\"global\");}}",
    ] {
        let guest = test(source, &["chosen"]);
        assert!(
            guest.status.success(),
            "{}",
            String::from_utf8_lossy(&guest.stderr)
        );
        assert_eq!(json(&guest)["results"][0]["status"], "passed");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/macos-first")
            .join(format!(
                "m013-sdk-panic-oracle-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        fs::write(root.join("oracle.rs"), source).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--test", "--crate-name=oracle"])
            .arg(root.join("oracle.rs"))
            .arg("-o")
            .arg(root.join("oracle"))
            .output()
            .unwrap();
        fs::write(root.join("compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("oracle"))
            .arg("chosen")
            .output()
            .unwrap();
        fs::write(root.join("native.stdout"), &native.stdout).unwrap();
        fs::write(root.join("native.stderr"), &native.stderr).unwrap();
        assert!(native.status.success());
        assert!(
            String::from_utf8(native.stdout)
                .unwrap()
                .contains("1 passed; 0 failed")
        );
    }
}

#[test]
fn rustc_private_module_test_is_visible_and_its_guest_panic_is_a_failure() {
    let source = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/run-unexported-tests.rs"
    );
    let listed = test(source, &["--list"]);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert_eq!(
        json(&listed)["listed"],
        serde_json::json!(["m::unexported"])
    );
    let ran = test(source, &["m::unexported", "--exact"]);
    assert_eq!(ran.status.code(), Some(1));
    assert_eq!(json(&ran)["results"][0]["status"], "failed");
    assert_eq!(
        json(&ran)["results"][0]["diagnostic"]["code"],
        "LRR-E-TEST-PANIC"
    );
}
#[test]
fn rustc_test_attribute_ignores_ordinary_macro_name_shadowing() {
    // Keep the original auxiliary unchanged. Inline its macro definition here
    // because standalone dependency-unit preparation is a separate CLI contract.
    let auxiliary = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/auxiliary/test_macro.rs"
    );
    let source = format!(
        "{auxiliary}\n#[test] fn foo(){{}} macro_rules! test{{()=>{{}}}} #[test] fn bar(){{}}"
    );
    let listed = test(&source, &["--list"]);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert_eq!(json(&listed)["listed"], serde_json::json!(["bar", "foo"]));
}

// LRR-specific generated-body/fault interaction; stock panic-message bodies
// retain their separate, unexecuted upstream acceptance requirements.
#[test]
fn generated_expected_guest_panic_matches_message_and_refuses_unsupported_fault() {
    let source = r#"macro_rules! tests { () => {
        #[test] #[should_panic(expected = "overflow")] fn matches() { let x=127i8; x+1i8; }
        #[test] #[should_panic(expected = "other")] fn mismatch() { let x=127i8; x+1i8; }
        #[test] #[should_panic] fn unsupported() { async {}; }
    }; } tests!();"#;
    let matches = test(source, &["matches", "--exact"]);
    assert!(
        matches.status.success(),
        "{}",
        String::from_utf8_lossy(&matches.stderr)
    );
    assert_eq!(json(&matches)["results"][0]["status"], "passed");
    for name in ["mismatch", "unsupported"] {
        let output = test(source, &[name, "--exact"]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(json(&output)["results"][0]["status"], "failed");
        if name == "unsupported" {
            assert_eq!(
                json(&output)["results"][0]["diagnostic"]["code"],
                "LRR-E-UNSUPPORTED"
            );
        }
    }
}

// Execute unchanged stock inputs. Native reference names are retained in the
// separate stock probe receipt; no source body is rewritten here.
#[test]
fn unchanged_rustc_test_catalog_sources_execute_all_selected_bodies() {
    let cases: &[(&str, &[&str])] = &[
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-20823.rs"
            ),
            &["foo"],
        ),
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-36768.rs"
            ),
            &["core", "foo"],
        ),
        (
            include_str!(
                "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-53675-a-test-called-panic.rs"
            ),
            &[
                "in_expression_position::panic",
                "in_statement_position::panic",
                "what_if_we_use_panic_directly_in_expr::panic",
                "what_if_we_use_panic_directly_in_stmt::panic",
            ],
        ),
    ];
    for (source, names) in cases {
        let output = test(source, &["--include-ignored"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = json(&output);
        let results = report["results"].as_array().unwrap();
        assert_eq!(results.len(), names.len());
        assert_eq!(
            results
                .iter()
                .map(|r| r["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            *names
        );
        assert!(
            results
                .iter()
                .all(|r| r["status"] == "passed" && r["diagnostic"].is_null())
        );
    }
}
// Minimal LRR namespace adaptation outside the unchanged licensed fixture:
// the stock public test body is retained exactly, and only an explicit compiler
// Attribute alias is added. Native Rust1.99 E0774 is a separate reference.
#[test]
fn rustc_public_test_source_cannot_borrow_compiler_derive_action_as_test() {
    let stock = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-20823.rs"
    );
    let source = format!("use core::prelude::rust_2024::derive as test;\n{stock}");
    for args in [
        vec!["--include-ignored", "--list"],
        vec!["foo", "--exact", "--list"],
        vec!["absent", "--exact", "--list"],
    ] {
        let output = test(&source, &args);
        assert_eq!(output.status.code(), Some(2));
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("LRR-E-CATALOG-INCOMPLETE"),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("LRR-E-TEST-NO-MATCH"));
        assert!(
            output.stdout.is_empty(),
            "incorrect compiler action published a catalog"
        );
    }
}
