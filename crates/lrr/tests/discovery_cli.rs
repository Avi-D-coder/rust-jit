use std::{
    fs,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(files: &[(&str, &str)]) -> Output {
    let root = std::env::temp_dir().join(format!(
        "lrr-discovery-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let result = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("run")
        .arg(root.join("main.rs"))
        .output()
        .unwrap();
    fs::remove_dir_all(root).unwrap();
    result
}
fn signed(output: Output, expected: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["semantics_profile"], "lrr-2024-v2");
    assert_eq!(report["value"]["kind"], "signed");
    assert_eq!(report["value"]["value"], expected);
    assert!(
        report["opcode_counters"]["operations_completed"]
            .as_u64()
            .unwrap()
            > 0
    );
}
#[test]
fn cli_generated_import_and_macro_reach_the_actual_fixed_point() {
    signed(
        run(&[(
            "main.rs",
            "#[macro_export]macro_rules! origin{()=>{pub fn answer()->i64{41_i64}}}macro_rules! bootstrap{()=>{use crate::origin as imported;macro_rules! later{()=>{imported!();}}}}bootstrap!();later!();fn main()->i64{answer()}fn cold<T:Missing>(){unknown();}",
        )]),
        "41",
    );
}
#[test]
fn cli_originating_definition_resolves_cyclic_reexports() {
    signed(
        run(&[(
            "main.rs",
            "mod a{pub use crate::b::answer;}mod b{pub use crate::a::answer;pub fn answer()->i64{43_i64}}fn main()->i64{a::answer()}",
        )]),
        "43",
    );
}
#[test]
fn cli_unresolved_macro_import_cycle_cannot_start_a_guest_run() {
    let output = run(&[(
        "main.rs",
        "use first as second;use second as first;first!();fn main()->i64{47_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
}
#[test]
fn cli_captured_external_generated_import_uses_actual_parent_body() {
    signed(
        run(&[
            (
                "main.rs",
                "macro_rules! choose{()=>{pub fn answer()->i64{53_i64}}}mod child;macro_rules! choose{()=>{pub fn answer()->i64{59_i64}}}fn main()->i64{child::answer()}",
            ),
            (
                "child.rs",
                "macro_rules! bootstrap{()=>{use choose as picked;picked!();}}bootstrap!();",
            ),
        ]),
        "53",
    );
}
#[test]
fn cli_path_attribute_keeps_logical_external_module_identity() {
    signed(
        run(&[
            (
                "main.rs",
                "#[path=\"other.rs\"]mod child;fn main()->i64{child::answer()}",
            ),
            ("other.rs", "pub fn answer()->i64{61_i64}"),
        ]),
        "61",
    );
}

#[test]
fn cli_generated_output_leaves_cold_body_errors_unprepared() {
    signed(
        run(&[(
            "main.rs",
            "macro_rules! make{()=>{pub fn answer()->i64{67_i64}fn cold<T:Missing>(){unknown::<Absent>();}}}make!();fn main()->i64{answer()}",
        )]),
        "67",
    );
}

#[test]
fn cli_failed_declarative_expansion_prevents_an_unrelated_main() {
    let output = run(&[(
        "main.rs",
        "macro_rules! make{(ok)=>{}}make!(bad);fn main()->i64{71_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-MACRO-EXPANSION"));
}

#[test]
fn cli_runtime_assertion_uses_the_completed_macro_scope() {
    let output = run(&[(
        "main.rs",
        "macro_rules! make{()=>{pub fn answer()->bool{true}}}make!();fn main(){assert!(answer());}",
    )]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["value"]["kind"], "unit");
}

#[test]
fn cli_inline_path_directory_does_not_become_a_logical_module_name() {
    signed(
        run(&[
            (
                "main.rs",
                "#[path=\"physical\"]mod outer{macro_rules! choose{()=>{pub fn answer()->i64{73_i64}}}pub mod child;}fn main()->i64{outer::child::answer()}",
            ),
            ("physical/child.rs", "choose!();"),
        ]),
        "73",
    );
}

#[test]
fn cli_completed_generated_macro_shadow_cannot_fall_back_to_a_builtin() {
    let output = run(&[(
        "main.rs",
        "macro_rules! make{()=>{macro_rules! assert{($e:expr)=>{}}}}make!();fn main(){assert!(true);}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
}

#[test]
fn native_oracle_agrees_with_cli_generated_import_and_macro_fixture() {
    let text = include_str!("../../../fixtures/macros/discovery/MX027/main.rs");
    let cli = run(&[("main.rs", text)]);
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(report["status"], "completed");
    assert_eq!(report["value"]["kind"], "unit");
    let root = std::env::temp_dir().join(format!(
        "lrr-discovery-native-oracle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("release: 1.99.0"));
    assert!(
        String::from_utf8_lossy(&version.stdout)
            .contains(&format!("host: {}", lrr::core::profile::GUEST_TARGET))
    );
    fs::write(root.join("oracle.rs"), text).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "-o"])
        .arg(root.join("oracle"))
        .arg(root.join("oracle.rs"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(root.join("oracle")).output().unwrap();
    assert!(native.status.success());
    assert!(native.stdout.is_empty());
    assert!(native.stderr.is_empty());
    let evidence = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/macos-first/m013-cli-oracles")
        .join(format!(
            "run-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&evidence).unwrap();
    for (name, bytes) in [
        ("rustc.stdout", &version.stdout),
        ("rustc.stderr", &version.stderr),
        ("build.stdout", &built.stdout),
        ("build.stderr", &built.stderr),
        ("native.stdout", &native.stdout),
        ("native.stderr", &native.stderr),
        ("cli.stdout", &cli.stdout),
        ("cli.stderr", &cli.stderr),
    ] {
        fs::write(evidence.join(name), bytes).unwrap();
    }
    fs::write(evidence.join("oracle.rs"), text).unwrap();
    fs::write(evidence.join("evidence.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "task":"M013", "platform":lrr::core::profile::GUEST_TARGET, "rust":"1.99.0",
        "fixture":"MX027", "oracle_preparation":true, "selected_live_unit_compiled":false,
        "rustc_args":["+1.99.0","--edition=2024","-o","<temporary oracle>","<isolated oracle.rs>"],
        "compiler_exit":built.status.code(), "native_exit":native.status.code(), "cli_exit":cli.status.code(),
        "scope":"CLI interpreter and explicitly compiled isolated native oracle; JIT mode matrix and full fixture acceptance remain pending",
        "release_complete":false,
    })).unwrap()).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_selects_a_generated_root_main_after_discovery() {
    signed(
        run(&[(
            "main.rs",
            "macro_rules! make{()=>{fn main()->i64{97_i64}fn cold<T:Missing>(){unknown();}}}make!();",
        )]),
        "97",
    );
}

#[test]
fn cli_nested_main_does_not_interfere_with_the_root_entry() {
    signed(
        run(&[(
            "main.rs",
            "mod nested{fn main(){unknown();}}fn main()->i64{101_i64}",
        )]),
        "101",
    );
}

#[test]
fn cli_nested_main_cannot_be_used_as_an_implicit_root_entry() {
    let output = run(&[("main.rs", "mod nested{fn main()->i64{103_i64}}")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-NAME-MISSING"));
}

#[test]
fn cli_generated_and_original_root_main_candidates_are_not_arbitrarily_selected() {
    let output = run(&[(
        "main.rs",
        "macro_rules! make{()=>{fn main()->i64{107_i64}}}make!();fn main()->i64{109_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-NAME-AMBIGUOUS"));
}

#[test]
fn cli_unresolved_producer_is_reported_before_a_missing_root_entry() {
    let output = run(&[("main.rs", "missing!();")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-CATALOG-INCOMPLETE"));
}

#[test]
fn cli_recursive_generated_entry_uses_actual_generated_callee_records() {
    signed(
        run(&[(
            "main.rs",
            "macro_rules! outer{()=>{macro_rules! inner{()=>{fn leaf()->i64{113_i64}fn main()->i64{leaf()}}}inner!();}}outer!();",
        )]),
        "113",
    );
}

#[test]
fn cli_distinct_generated_main_records_remain_ambiguous() {
    let output = run(&[(
        "main.rs",
        "macro_rules! make{()=>{fn main()->i64{127_i64}}}make!();make!();",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-NAME-AMBIGUOUS"));
}

#[test]
fn cli_cfg_disabled_generated_main_is_a_certified_missing_entry() {
    let output = run(&[(
        "main.rs",
        "macro_rules! make{()=>{#[cfg(any())]fn main()->i64{131_i64}}}make!();",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LRR-E-NAME-MISSING"));
}

#[test]
fn cli_captured_macro_use_generated_export_preserves_completed_index() {
    signed(
        run(&[
            (
                "main.rs",
                "#[macro_use]mod child;leaf!();fn main()->i64{made()}",
            ),
            (
                "child.rs",
                "macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn made()->i64{577_i64}}}}}seed!();",
            ),
        ]),
        "577",
    );
}

#[test]
fn cli_assertion_predicate_lexical_shadow_never_falls_back_to_module() {
    for text in [
        "fn answer()->bool{panic!(\"wrong module\")}fn main(){fn answer()->bool{true}assert!(answer());}",
        "fn answer()->bool{panic!(\"wrong module\")}fn main(){let answer=true;assert!(answer());}",
    ] {
        let output = run(&[("main.rs", text)]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("LRR-E-UNSUPPORTED"), "{error}");
        assert!(
            error.contains("assertion predicate requires lexical callable context integration"),
            "{error}"
        );
        // Source diagnostics may quote the cold body; typed refusal/status is authority.
    }
}
