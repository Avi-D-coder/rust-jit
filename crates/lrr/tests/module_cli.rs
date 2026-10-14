//! M013: actual CLI capture/adoption, physical layouts, and refused frontiers.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture(files: &[(&str, &str)]) -> (PathBuf, Output) {
    let sequence = NEXT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/macos-first")
        .join(format!("m013-module-cli-{}-{sequence}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("run")
        .arg(root.join("main.rs"))
        .output()
        .unwrap();
    fs::write(root.join("cli.stdout"), &output.stdout).unwrap();
    fs::write(root.join("cli.stderr"), &output.stderr).unwrap();
    (root, output)
}
fn value(output: &Output) -> String {
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
    report["value"]["value"].as_str().unwrap().into()
}
fn refused(output: &Output, code: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn cli_adopted_source_ordinary_grandchild_matches_native_witness() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            include_str!("../../../artifacts/macos-first/m013-ordinary-module-witness/main.rs"),
        ),
        (
            "child.rs",
            include_str!("../../../artifacts/macos-first/m013-ordinary-module-witness/child.rs"),
        ),
        (
            "child/grandchild.rs",
            include_str!(
                "../../../artifacts/macos-first/m013-ordinary-module-witness/child/grandchild.rs"
            ),
        ),
    ]);
    assert_eq!(value(&output), "2053");
    compare_module_native(&root, &output);
    let mut comparison: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("inline-comparison.json")).unwrap()).unwrap();
    comparison["scope"] = "Actual CLI capture/adoption, completion and publication of an ordinary grandchild within an adopted generated child, matching the historical 2053 witness; full discovery fixtures and release remain pending".into();
    comparison["historical_witness"] =
        "artifacts/macos-first/m013-ordinary-module-witness/witness.json".into();
    fs::write(
        root.join("ordinary-comparison.json"),
        serde_json::to_vec_pretty(&comparison).unwrap(),
    )
    .unwrap();
}
#[test]
fn cli_adopted_source_ordinary_modules_preserve_nested_physical_layout() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "#[path=\"physical/chosen.rs\"]mod grandchild;pub fn made()->i64{grandchild::made()}",
        ),
        (
            "physical/chosen.rs",
            "mod leaf;pub fn made()->i64{leaf::made()}",
        ),
        ("physical/leaf.rs", "pub fn made()->i64{2063_i64}"),
        ("child/grandchild.rs", "pub fn made()->i64{999_i64}"),
    ]);
    assert_eq!(value(&output), "2063");
    compare_module_native(&root, &output);
}
#[test]
fn cli_adopted_source_ordinary_missing_child_refuses_before_guest_run() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{7_i64}",
        ),
        ("child.rs", "mod absent;pub fn made()->i64{1_i64}"),
    ]);
    refused(&output, "LRR-E-SOURCE-MISSING");
}
#[test]
fn cli_adopted_source_ordinary_runtime_assertions_use_completed_context() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "mod grandchild;pub fn made()->i64{grandchild::made()}",
        ),
        (
            "child/grandchild.rs",
            "pub fn made()->i64{assert!(true);2071_i64}",
        ),
    ]);
    assert_eq!(value(&output), "2071");
    compare_module_native(&root, &output);
}
#[test]
fn cli_ordinary_adopted_macro_inherits_actual_parent_definition_cutoff() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::grandchild::made()}",
        ),
        (
            "child.rs",
            "macro_rules! selected{()=>{pub fn made()->i64{2081_i64}}}pub mod grandchild;macro_rules! selected{()=>{pub fn made()->i64{999_i64}}}",
        ),
        ("child/grandchild.rs", "selected!();"),
    ]);
    assert_eq!(value(&output), "2081");
    compare_module_native(&root, &output);
}
#[test]
fn cli_ordinary_adopted_local_macro_shadows_parent_with_generated_child_call() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::grandchild::made()}",
        ),
        (
            "child.rs",
            "macro_rules! selected{()=>{pub fn made()->i64{999_i64}}}pub mod grandchild;",
        ),
        (
            "child/grandchild.rs",
            "macro_rules! selected{()=>{macro_rules! nested{()=>{pub fn made()->i64{2083_i64}}}nested!();}}selected!();",
        ),
    ]);
    assert_eq!(value(&output), "2083");
    compare_module_native(&root, &output);
}
#[test]
fn cli_ordinary_adopted_macro_cannot_capture_future_parent_definition() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{7_i64}",
        ),
        (
            "child.rs",
            "mod grandchild;macro_rules! later{()=>{pub fn made(){}}}",
        ),
        ("child/grandchild.rs", "later!();"),
    ]);
    refused(&output, "LRR-E-CATALOG-INCOMPLETE");
}
#[test]
fn cli_ordinary_adopted_compiler_include_uses_physical_source_and_parent_operand() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::grandchild::made()}",
        ),
        (
            "child.rs",
            "macro_rules! chosen{()=>{\"made.rs\"}}pub mod grandchild;macro_rules! chosen{()=>{\"wrong.rs\"}}",
        ),
        ("child/grandchild.rs", "include!(chosen!());"),
        ("child/made.rs", "pub fn made()->i64{2087_i64}"),
        ("made.rs", "pub fn made()->i64{999_i64}"),
    ]);
    assert_eq!(value(&output), "2087");
    compare_module_native(&root, &output);
}
#[test]
fn cli_ordinary_adopted_macro_use_retains_actual_child_body_owner() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "#[macro_use]mod grandchild;selected!();"),
        (
            "child/grandchild.rs",
            "macro_rules! selected{()=>{pub fn made()->i64{2089_i64}}}",
        ),
    ]);
    assert_eq!(value(&output), "2089");
    compare_module_native(&root, &output);
}
#[test]
fn cli_ordinary_adopted_generated_module_keeps_actual_ordinary_caller() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::grandchild::leaf::made()}",
        ),
        ("child.rs", "pub mod grandchild;"),
        (
            "child/grandchild.rs",
            "macro_rules! selected{()=>{pub mod leaf;}}selected!();",
        ),
        ("child/grandchild/leaf.rs", "pub fn made()->i64{2099_i64}"),
        ("leaf.rs", "pub fn made()->i64{999_i64}"),
    ]);
    assert_eq!(value(&output), "2099");
    compare_module_native(&root, &output);
}
#[test]
fn cli_ordinary_adopted_exported_macro_reaches_original_root_call() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]\nmacro_rules! emit{()=>{mod child;}}emit!();crate::selected!();fn main()->i64{made()}",
        ),
        ("child.rs", "mod grandchild;"),
        (
            "child/grandchild.rs",
            "#[macro_export]macro_rules! selected{()=>{fn made()->i64{2111_i64}}}",
        ),
    ]);
    assert_eq!(value(&output), "2111");
    compare_module_native(&root, &output);
}
#[test]
fn mx027_discovery_generated_import_and_macro_reach_fixed_point() {
    let (root, output) = fixture(&[(
        "main.rs",
        "#[macro_export]macro_rules! origin{()=>{pub fn made()->i64{2129_i64}}}macro_rules! seed{()=>{use crate::origin as imported;macro_rules! later{()=>{imported!();}}}}seed!();later!();fn main()->i64{made()}",
    )]);
    assert_eq!(value(&output), "2129");
    compare_module_native(&root, &output);
}
#[test]
fn mx028_discovery_refuses_unexpanded_cycle_and_resolves_originating_reexports() {
    let (_, refused_output) = fixture(&[(
        "main.rs",
        "use first as second;use second as first;first!();fn main()->i64{7_i64}",
    )]);
    refused(&refused_output, "LRR-E-CATALOG-INCOMPLETE");
    let (root, output) = fixture(&[(
        "main.rs",
        "#[macro_export]macro_rules! origin{()=>{fn made()->i64{2131_i64}}}mod a{pub use crate::b::*;}mod b{pub use crate::a::*;pub use crate::origin as selected;}a::selected!();fn main()->i64{made()}",
    )]);
    assert_eq!(value(&output), "2131");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_external_module_matches_actual_native_macos() {
    let live = include_str!("../../../artifacts/macos-first/m013-generated-module-probe/live.rs");
    let child = include_str!("../../../artifacts/macos-first/m013-generated-module-probe/child.rs");
    let (root, cli) = fixture(&[("main.rs", live), ("child.rs", child)]);
    let actual = value(&cli);
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
    // Explicit oracle preparation compiles an isolated copy after the live CLI run.
    let oracle = live.replace("fn main()->i64", "fn case()->i64")
        + "\nfn main(){::std::println!(\"{}\",case());}\n";
    fs::write(root.join("oracle.rs"), &oracle).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
        .arg(root.join("oracle.rs"))
        .arg("-o")
        .arg(root.join("oracle"))
        .output()
        .unwrap();
    fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
    fs::write(root.join("compile.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(root.join("oracle")).output().unwrap();
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    fs::write(root.join("native.stderr"), &native.stderr).unwrap();
    assert!(native.status.success());
    assert!(native.stderr.is_empty());
    let expected = String::from_utf8(native.stdout).unwrap();
    assert_eq!(actual, expected.trim());
    fs::write(root.join("comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "task":"M013", "platform":"aarch64-apple-darwin", "rust_version":"1.99.0",
        "comparison_passed":true, "cli_exit_code":cli.status.code(), "native_exit_code":native.status.code(),
        "cli_value":actual, "native_stdout":expected, "selected_live_unit_compiled":false,
        "release_complete":false,
        "historical_witness":"artifacts/macos-first/m013-module-frontier-native-witness.json",
        "scope":"Actual CLI interpreter capture/adoption and execution of the historical simple generated external-module witness; full fixture, service, nested layout, JIT and release acceptance remain pending"
    })).unwrap()).unwrap();
}
#[test]
fn cli_generated_module_uses_captured_nonroot_file_layout() {
    let (_, output) = fixture(&[
        ("main.rs", "mod outer;fn main()->i64{outer::child::made()}"),
        (
            "outer.rs",
            "macro_rules! emit{()=>{pub mod child;}}emit!();",
        ),
        ("outer/child.rs", "pub fn made()->i64{1601_i64}"),
        ("child.rs", "pub fn made()->i64{999_i64}"),
    ]);
    assert_eq!(value(&output), "1601");
}
#[test]
fn cli_generated_module_path_override_preserves_physical_caller_layout() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#[path=\"physical/outer.rs\"]mod outer;fn main()->i64{outer::child::made()}",
        ),
        (
            "physical/outer.rs",
            "macro_rules! emit{()=>{#[path=\"chosen.rs\"]pub mod child;}}emit!();",
        ),
        ("physical/chosen.rs", "pub fn made()->i64{1607_i64}"),
        ("chosen.rs", "pub fn made()->i64{991_i64}"),
    ]);
    assert_eq!(value(&output), "1607");
}
#[test]
fn cli_missing_generated_module_prevents_unrelated_entry_execution() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{1613_i64}",
    )]);
    refused(&output, "LRR-E-SOURCE-MISSING");
}
#[test]
fn cli_ambiguous_generated_module_prevents_unrelated_entry_execution() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{1619_i64}",
        ),
        ("child.rs", "pub fn made()->i64{1_i64}"),
        ("child/mod.rs", "pub fn made()->i64{2_i64}"),
    ]);
    refused(&output, "LRR-E-MODULE-AMBIGUOUS");
}
#[test]
fn cli_unknown_macro_in_adopted_module_keeps_namespace_incomplete() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{1621_i64}",
        ),
        ("child.rs", "missing!();pub fn made()->i64{1_i64}"),
    ]);
    refused(&output, "LRR-E-CATALOG-INCOMPLETE");
}
#[test]
fn cli_adopted_module_expands_actual_parent_macro_body() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! make{()=>{pub fn made()->i64{1627_i64}}}macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "make!();"),
    ]);
    assert_eq!(value(&output), "1627");
}
#[test]
fn cli_generated_cfg_disabled_module_requires_no_file_capture() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! emit{()=>{#[cfg(any())]mod absent;}}emit!();fn main()->i64{1637_i64}",
    )]);
    assert_eq!(value(&output), "1637");
}

#[test]
fn cli_generated_module_in_original_inline_scope_uses_actual_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "mod outer{macro_rules! emit{()=>{pub mod child;}}emit!();}fn main()->i64{outer::child::made()}",
        ),
        ("outer/child.rs", "pub fn made()->i64{1657_i64}"),
        ("child.rs", "pub fn made()->i64{977_i64}"),
    ]);
    assert_eq!(value(&output), "1657");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_path_override_in_inline_scope_stays_relative_to_inline_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "mod outer{macro_rules! emit{()=>{#[path=\"chosen.rs\"]pub mod child;}}emit!();}fn main()->i64{outer::child::made()}",
        ),
        ("outer/chosen.rs", "pub fn made()->i64{1663_i64}"),
        ("chosen.rs", "pub fn made()->i64{971_i64}"),
    ]);
    assert_eq!(value(&output), "1663");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_module_keeps_nested_inline_path_attributes() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#[path=\"physical\"]mod outer{#[path=\"deep\"]mod inner{macro_rules! emit{()=>{pub mod child;}}emit!();}pub fn answer()->i64{inner::child::made()}}fn main()->i64{outer::answer()}",
        ),
        ("physical/deep/child.rs", "pub fn made()->i64{1667_i64}"),
        ("outer/inner/child.rs", "pub fn made()->i64{967_i64}"),
    ]);
    assert_eq!(value(&output), "1667");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_module_in_nonroot_inline_scope_keeps_file_module_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "mod parent;fn main()->i64{parent::outer::child::made()}",
        ),
        (
            "parent.rs",
            "pub mod outer{macro_rules! emit{()=>{pub mod child;}}emit!();}",
        ),
        ("parent/outer/child.rs", "pub fn made()->i64{1669_i64}"),
        ("outer/child.rs", "pub fn made()->i64{953_i64}"),
    ]);
    assert_eq!(value(&output), "1669");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_module_in_nonroot_inline_path_scope_keeps_source_file_parent() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#[path=\"physical/parent.rs\"]mod parent;fn main()->i64{parent::outer::child::made()}",
        ),
        (
            "physical/parent.rs",
            "#[path=\"chosen\"]pub mod outer{macro_rules! emit{()=>{pub mod child;}}emit!();}",
        ),
        ("physical/chosen/child.rs", "pub fn made()->i64{1693_i64}"),
        (
            "physical/parent/chosen/child.rs",
            "pub fn made()->i64{947_i64}",
        ),
    ]);
    assert_eq!(value(&output), "1693");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_inline_layout_cannot_escape_the_capture_authority() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[path=\"..\"]mod outer{macro_rules! emit{()=>{mod child;}}emit!();}fn main()->i64{1697_i64}",
    )]);
    refused(&output, "LRR-E-SOURCE-PATH");
}
fn compare_module_native(root: &std::path::Path, cli: &Output) {
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
    let live = fs::read_to_string(root.join("main.rs")).unwrap();
    let oracle = live.replace("fn main()->i64", "fn case()->i64")
        + "\nfn main(){::std::println!(\"{}\",case());}\n";
    fs::write(root.join("oracle.rs"), oracle).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
        .arg(root.join("oracle.rs"))
        .arg("-o")
        .arg(root.join("oracle"))
        .output()
        .unwrap();
    fs::write(root.join("rustc.stdout"), version.stdout).unwrap();
    fs::write(root.join("compile.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(root.join("oracle")).output().unwrap();
    fs::write(root.join("native.stdout"), &native.stdout).unwrap();
    fs::write(root.join("native.stderr"), &native.stderr).unwrap();
    assert!(native.status.success());
    assert!(native.stderr.is_empty());
    let observed = String::from_utf8(native.stdout).unwrap();
    let actual = value(cli);
    assert_eq!(actual, observed.trim());
    fs::write(root.join("inline-comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "task":"M013", "platform":"aarch64-apple-darwin", "rust_version":"1.99.0",
        "comparison_passed":true, "cli_exit_code":cli.status.code(), "native_exit_code":native.status.code(),
        "cli_value":actual, "native_stdout":observed, "selected_live_unit_compiled":false,
        "release_complete":false,
        "scope":"Actual CLI interpreter physical layout for macro-emitted modules in original inline scopes versus explicitly compiled isolated native oracle copies; nested generated/physical module contexts, full fixture, JIT and release acceptance remain pending"
    })).unwrap()).unwrap();
}

#[test]
fn cli_generated_inline_output_combines_with_original_inline_layout() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "mod outer{macro_rules! emit{()=>{#[path=\"generated\"]pub mod middle{pub mod child;}}}emit!();}fn main()->i64{outer::middle::child::made()}",
        ),
        ("outer/generated/child.rs", "pub fn made()->i64{1699_i64}"),
        ("generated/child.rs", "pub fn made()->i64{941_i64}"),
    ]);
    assert_eq!(value(&output), "1699");
    compare_module_native(&root, &output);
}
#[test]
fn cli_generated_inline_layout_uses_cfg_active_parent_path_attributes() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#[cfg(any())]mod outer{missing!();}#[cfg_attr(unix,path=\"physical\")]mod outer{macro_rules! emit{()=>{pub mod child;}}emit!();}fn main()->i64{outer::child::made()}",
        ),
        ("physical/child.rs", "pub fn made()->i64{1709_i64}"),
        ("outer/child.rs", "pub fn made()->i64{937_i64}"),
    ]);
    assert_eq!(value(&output), "1709");
    compare_module_native(&root, &output);
}

#[test]
fn cli_generated_inline_layout_preserves_canonical_unicode_and_raw_module_names() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#[path=\"physical\"]mod cafe\u{301}{pub mod r#type{macro_rules! emit{()=>{pub mod child;}}emit!();}}fn main()->i64{café::r#type::child::made()}",
        ),
        ("physical/type/child.rs", "pub fn made()->i64{1721_i64}"),
    ]);
    assert_eq!(value(&output), "1721");
    compare_module_native(&root, &output);
}

#[test]
fn cli_later_generated_inline_caller_uses_retained_parent_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{pub mod outer{macro_rules! next{()=>{pub mod child;}}next!();}}}emit!();fn main()->i64{outer::child::made()}",
        ),
        ("outer/child.rs", "pub fn made()->i64{1741_i64}"),
        ("child.rs", "pub fn made()->i64{919_i64}"),
    ]);
    assert_eq!(value(&output), "1741");
    compare_generated_module_native(&root, &output);
}
#[test]
fn cli_later_generated_path_caller_combines_original_inline_context() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#[path=\"physical\"]mod first{macro_rules! emit{()=>{#[path=\"generated\"]pub mod outer{macro_rules! next{()=>{#[path=\"chosen.rs\"]pub mod child;}}next!();}}}emit!();}fn main()->i64{first::outer::child::made()}",
        ),
        (
            "physical/generated/chosen.rs",
            "pub fn made()->i64{1747_i64}",
        ),
        ("generated/chosen.rs", "pub fn made()->i64{911_i64}"),
        ("physical/chosen.rs", "pub fn made()->i64{907_i64}"),
    ]);
    assert_eq!(value(&output), "1747");
    compare_generated_module_native(&root, &output);
}
#[test]
fn cli_later_generated_call_keeps_each_nested_expansion_layout() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{#[path=\"physical\"]pub mod outer{macro_rules! middle{()=>{#[path=\"deep\"]pub mod inner{macro_rules! last{()=>{pub mod child;}}last!();}}}middle!();}}}emit!();fn main()->i64{outer::inner::child::made()}",
        ),
        ("physical/deep/child.rs", "pub fn made()->i64{1753_i64}"),
        ("outer/inner/child.rs", "pub fn made()->i64{887_i64}"),
        ("deep/child.rs", "pub fn made()->i64{883_i64}"),
    ]);
    assert_eq!(value(&output), "1753");
    compare_generated_module_native(&root, &output);
}
#[test]
fn cli_generated_sibling_callers_keep_distinct_directories_and_parent_cutoffs() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! next{()=>{pub mod child;}}macro_rules! helper{()=>{pub fn made()->i64{1783_i64}}}pub mod left{next!();}macro_rules! helper{()=>{pub fn made()->i64{1787_i64}}}pub mod right{next!();}}}emit!();fn main()->i64{left::child::made()+right::child::made()}",
        ),
        ("left/child.rs", "helper!();"),
        ("right/child.rs", "helper!();"),
        ("child.rs", "pub fn made()->i64{877_i64}"),
    ]);
    assert_eq!(value(&output), "3570");
    compare_generated_module_native(&root, &output);
}
#[test]
fn cli_generated_inline_caller_in_nonroot_file_keeps_physical_module_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "mod parent;fn main()->i64{parent::outer::child::made()}",
        ),
        (
            "parent.rs",
            "macro_rules! emit{()=>{pub mod outer{macro_rules! next{()=>{pub mod child;}}next!();}}}emit!();",
        ),
        ("parent/outer/child.rs", "pub fn made()->i64{1789_i64}"),
        ("outer/child.rs", "pub fn made()->i64{863_i64}"),
    ]);
    assert_eq!(value(&output), "1789");
    compare_generated_module_native(&root, &output);
}
#[test]
fn cli_generated_inline_caller_retains_cfg_unicode_and_raw_name_identity() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{#[cfg(any())]mod café{missing!();}#[cfg_attr(unix,path=\"physical\")]pub mod cafe\u{301}{pub mod r#type{macro_rules! next{()=>{pub mod child;}}next!();}}}}emit!();fn main()->i64{café::r#type::child::made()}",
        ),
        ("physical/type/child.rs", "pub fn made()->i64{1801_i64}"),
    ]);
    assert_eq!(value(&output), "1801");
    compare_generated_module_native(&root, &output);
}
#[test]
fn cli_generated_inline_caller_cannot_escape_source_authority() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! emit{()=>{#[path=\"..\"]mod outer{macro_rules! next{()=>{mod child;}}next!();}}}emit!();fn main()->i64{1811_i64}",
    )]);
    refused(&output, "LRR-E-SOURCE-PATH");
}
fn compare_generated_module_native(root: &std::path::Path, cli: &Output) {
    compare_module_native(root, cli);
    let path = root.join("inline-comparison.json");
    let mut comparison: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    comparison["scope"] = "Actual CLI interpreter caller layout through retained generated expansion ancestry versus an explicitly compiled isolated native oracle; adopted nested physical contexts, full fixture/JIT/release remain pending".into();
    fs::write(&path, serde_json::to_vec_pretty(&comparison).unwrap()).unwrap();
    fs::write(
        root.join("generated-comparison.json"),
        serde_json::to_vec_pretty(&comparison).unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_adopted_child_runtime_assertion_uses_its_actual_body_source() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "pub fn made()->i64{assert!(true);1831_i64}"),
    ]);
    assert_eq!(value(&output), "1831");
    compare_runtime_child_native(&root, &output);
}
#[test]
fn cli_adopted_child_runtime_alias_uses_sealed_child_imports() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]\nextern crate core;macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "use crate::core::assert as checked;pub fn made()->i64{checked!(true);1847_i64}",
        ),
    ]);
    assert_eq!(value(&output), "1847");
    compare_runtime_child_native(&root, &output);
}
#[test]
fn cli_adopted_child_runtime_prelude_disable_stays_in_its_actual_scope() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{assert!(true);child::made()}",
        ),
        (
            "child.rs",
            "#![no_implicit_prelude]\nextern crate core;pub fn made()->i64{core::assert!(true);1861_i64}",
        ),
    ]);
    assert_eq!(value(&output), "1861");
    compare_runtime_child_native(&root, &output);
}
#[test]
fn cli_later_parent_runtime_shadow_does_not_change_adopted_child_cutoff() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();macro_rules! assert{($e:expr)=>{}}fn main()->i64{child::made()}",
        ),
        ("child.rs", "pub fn made()->i64{assert!(true);1867_i64}"),
    ]);
    assert_eq!(value(&output), "1867");
    compare_runtime_child_native(&root, &output);
}
#[test]
fn cli_adopted_child_runtime_false_assertion_reports_actual_panic() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "pub fn made()->i64{assert!(false);1871_i64}"),
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "guest_panic");
    assert_eq!(report["panic"]["message"], "assertion failed: false");
}
#[test]
fn cli_adopted_child_runtime_implicit_disable_refuses_bare_sdk_macro() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "#![no_implicit_prelude]\npub fn made()->i64{assert_eq!(1_i64,1_i64);1873_i64}",
        ),
    ]);
    refused(&output, "LRR-E-MACRO-ABSENT");
}
#[test]
fn cli_generated_parent_runtime_shadow_cannot_fall_back_to_compiler_macro() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! assert{($e:expr)=>{}}mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "pub fn made()->i64{assert!(false);1877_i64}"),
    ]);
    refused(&output, "LRR-E-CATALOG-INCOMPLETE");
}
fn compare_runtime_child_native(root: &std::path::Path, cli: &Output) {
    compare_module_native(root, cli);
    let path = root.join("inline-comparison.json");
    let mut comparison: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    comparison["scope"] = "Actual CLI interpreter runtime compiler-macro routing in adopted child bodies versus explicitly compiled isolated native copies; user body macro expansion, full fixtures/JIT/release remain pending".into();
    fs::write(&path, serde_json::to_vec_pretty(&comparison).unwrap()).unwrap();
    fs::write(
        root.join("runtime-comparison.json"),
        serde_json::to_vec_pretty(&comparison).unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_many_captured_plain_modules_do_not_consume_runtime_macro_contexts() {
    let mut files = Vec::new();
    let mut main = String::new();
    for n in 0..1040 {
        main.push_str(&format!("mod c{n};"));
        files.push((
            format!("c{n}.rs"),
            format!("pub fn made()->i64{{{}_i64}}", 1900 + n),
        ));
    }
    main.push_str("fn main()->i64{assert!(true);c1039::made()}");
    files.insert(0, ("main.rs".into(), main));
    let borrowed = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    let (_, output) = fixture(&borrowed);
    assert_eq!(value(&output), "2939");
}
#[test]
fn cli_original_parent_runtime_shadow_cannot_fall_back_in_adopted_child() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! assert{($e:expr)=>{}}macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "pub fn made()->i64{assert!(true);1889_i64}"),
    ]);
    refused(&output, "LRR-E-CATALOG-INCOMPLETE");
}

#[test]
fn cli_adopted_child_literal_include_uses_actual_physical_file() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child/mod.rs", "include!(\"part.rs\");"),
        ("child/part.rs", "pub fn made()->i64{1901_i64}"),
        ("part.rs", "pub fn made()->i64{853_i64}"),
    ]);
    assert_eq!(value(&output), "1901");
    compare_child_compiler_native(&root, &output);
}
#[test]
fn cli_adopted_child_include_concat_uses_compiler_identity_and_child_scope() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "include!(concat!(\"part\",\".rs\"));"),
        ("part.rs", "pub fn made()->i64{1907_i64}"),
    ]);
    assert_eq!(value(&output), "1907");
    compare_child_compiler_native(&root, &output);
}
#[test]
fn cli_adopted_child_missing_include_refuses_before_entry() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{1913_i64}",
        ),
        ("child.rs", "include!(\"absent.rs\");"),
    ]);
    refused(&output, "LRR-E-SOURCE-PATH");
}
#[test]
fn cli_adopted_child_include_cannot_escape_source_authority() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{1921_i64}",
        ),
        ("child.rs", "include!(\"../outside.rs\");"),
    ]);
    refused(&output, "LRR-E-SOURCE-PATH");
}
#[test]
fn cli_adopted_child_source_include_shadow_never_captures_unused_file() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "macro_rules! include{($p:expr)=>{pub fn made()->i64{1931_i64}}}include!(\"absent.rs\");",
        ),
    ]);
    assert_eq!(value(&output), "1931");
    compare_child_compiler_native(&root, &output);
}
fn compare_child_compiler_native(root: &std::path::Path, cli: &Output) {
    compare_module_native(root, cli);
    let path = root.join("inline-comparison.json");
    let mut comparison: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    comparison["scope"] = "Actual CLI interpreter explicit compiler item expansion/capture/adoption in adopted child files versus isolated native copies; broader inherited/generated/nested contexts, body compiler macros and full fixture/JIT/release remain pending".into();
    fs::write(&path, serde_json::to_vec_pretty(&comparison).unwrap()).unwrap();
    fs::write(
        root.join("child-compiler-comparison.json"),
        serde_json::to_vec_pretty(&comparison).unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_adopted_child_local_concat_shadow_executes_selected_declaration() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "macro_rules! concat{($($x:tt)*)=>{\"selected.rs\"}}include!(concat!(\"wrong.rs\"));",
        ),
        ("selected.rs", "pub fn made()->i64{1949_i64}"),
    ]);
    assert_eq!(value(&output), "1949");
    compare_child_compiler_native(&root, &output);
}
#[test]
fn cli_adopted_child_original_concat_shadow_keeps_parent_cutoff() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! concat{($($x:tt)*)=>{\"selected.rs\"}}macro_rules! emit{()=>{mod child;}}emit!();macro_rules! concat{($($x:tt)*)=>{\"later.rs\"}}fn main()->i64{child::made()}",
        ),
        ("child.rs", "include!(concat!(\"wrong.rs\"));"),
        ("selected.rs", "pub fn made()->i64{1951_i64}"),
        ("later.rs", "pub fn made()->i64{857_i64}"),
    ]);
    assert_eq!(value(&output), "1951");
    compare_child_compiler_native(&root, &output);
}
#[test]
fn cli_adopted_child_generated_concat_shadow_keeps_generated_parent() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! concat{($x:literal)=>{\"selected.rs\"}}mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "include!(concat!(\"wrong.rs\"));"),
        ("selected.rs", "pub fn made()->i64{1957_i64}"),
    ]);
    assert_eq!(value(&output), "1957");
    compare_child_compiler_native(&root, &output);
}

#[test]
fn cli_child_generated_include_retains_physical_child_base() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child/mod.rs",
            "macro_rules! load{()=>{include!(\"part.rs\");}}load!();",
        ),
        ("child/part.rs", "pub fn made()->i64{1973_i64}"),
        ("part.rs", "pub fn made()->i64{859_i64}"),
    ]);
    assert_eq!(value(&output), "1973");
    compare_child_generated_compiler_native(&root, &output);
}
#[test]
fn cli_child_nested_generated_include_keeps_every_caller_frame() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child/mod.rs",
            "macro_rules! load{()=>{macro_rules! next{()=>{include!(concat!(\"part\",\".rs\"));}}next!();}}load!();",
        ),
        ("child/part.rs", "pub fn made()->i64{1979_i64}"),
        ("part.rs", "pub fn made()->i64{863_i64}"),
    ]);
    assert_eq!(value(&output), "1979");
    compare_child_generated_compiler_native(&root, &output);
}
#[test]
fn cli_child_generated_include_preserves_original_operand_cutoff() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! concat{($x:literal)=>{\"part.rs\"}}macro_rules! emit{()=>{mod child;}}emit!();macro_rules! concat{($x:literal)=>{\"later.rs\"}}fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "macro_rules! load{()=>{include!(concat!(\"wrong.rs\"));}}load!();",
        ),
        ("part.rs", "pub fn made()->i64{1987_i64}"),
        ("later.rs", "pub fn made()->i64{877_i64}"),
    ]);
    assert_eq!(value(&output), "1987");
    compare_child_generated_compiler_native(&root, &output);
}
#[test]
fn cli_child_generated_include_preserves_generated_parent_operand() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! concat{($x:literal)=>{\"part.rs\"}}mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "macro_rules! load{()=>{include!(concat!(\"wrong.rs\"));}}load!();",
        ),
        ("part.rs", "pub fn made()->i64{1993_i64}"),
    ]);
    assert_eq!(value(&output), "1993");
    compare_child_generated_compiler_native(&root, &output);
}
fn compare_child_generated_compiler_native(root: &std::path::Path, cli: &Output) {
    compare_child_compiler_native(root, cli);
    let path = root.join("child-compiler-comparison.json");
    let mut report: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    report["scope"] = "Actual CLI compiler item macros emitted by declarative expansions in adopted physical children versus isolated native copies; inherited original/generated cutoffs and physical source provenance; full fixture/JIT/release pending".into();
    fs::write(
        root.join("child-generated-compiler-comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_child_nested_include_uses_each_included_file_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "include!(\"pieces/first.rs\");"),
        ("pieces/first.rs", "include!(concat!(\"second\",\".rs\"));"),
        ("pieces/second.rs", "pub fn made()->i64{2003_i64}"),
        ("second.rs", "pub fn made()->i64{881_i64}"),
    ]);
    assert_eq!(value(&output), "2003");
    compare_child_included_compiler_native(&root, &output);
}
#[test]
fn cli_child_included_generated_call_retains_included_file_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        ("child.rs", "include!(\"pieces/first.rs\");"),
        (
            "pieces/first.rs",
            "macro_rules! next{()=>{include!(\"second.rs\");}}next!();",
        ),
        ("pieces/second.rs", "pub fn made()->i64{2011_i64}"),
        ("second.rs", "pub fn made()->i64{883_i64}"),
    ]);
    assert_eq!(value(&output), "2011");
    compare_child_included_compiler_native(&root, &output);
}
#[test]
fn cli_child_included_operand_shadow_preserves_original_parent_cutoff() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! concat{($x:literal)=>{\"second.rs\"}}macro_rules! emit{()=>{mod child;}}emit!();macro_rules! concat{($x:literal)=>{\"wrong.rs\"}}fn main()->i64{child::made()}",
        ),
        ("child.rs", "include!(\"pieces/first.rs\");"),
        ("pieces/first.rs", "include!(concat!(\"ignored.rs\"));"),
        ("pieces/second.rs", "pub fn made()->i64{2017_i64}"),
        ("second.rs", "pub fn made()->i64{887_i64}"),
    ]);
    assert_eq!(value(&output), "2017");
    compare_child_included_compiler_native(&root, &output);
}
fn compare_child_included_compiler_native(root: &std::path::Path, cli: &Output) {
    compare_child_compiler_native(root, cli);
    let path = root.join("child-compiler-comparison.json");
    let mut report: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    report["scope"] = "Actual CLI nested compiler item expansion in included adopted child sources versus isolated native copies; each included physical directory and inherited lexical cutoff; full fixture/JIT/release pending".into();
    fs::write(
        root.join("child-included-compiler-comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_adopted_child_generated_grandchild_uses_actual_parent_source() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "macro_rules! emit{()=>{mod grandchild;}}emit!();pub fn made()->i64{grandchild::made()}",
        ),
        ("child/grandchild.rs", "pub fn made()->i64{2027_i64}"),
        ("grandchild.rs", "pub fn made()->i64{907_i64}"),
    ]);
    assert_eq!(value(&output), "2027");
    compare_nested_physical_native(&root, &output);
}
#[test]
fn cli_nested_adopted_modules_keep_each_physical_directory() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child/mod.rs",
            "macro_rules! emit{()=>{mod grandchild;}}emit!();pub fn made()->i64{grandchild::made()}",
        ),
        (
            "child/grandchild.rs",
            "macro_rules! emit{()=>{mod leaf;}}emit!();pub fn made()->i64{leaf::made()}",
        ),
        ("child/grandchild/leaf.rs", "pub fn made()->i64{2029_i64}"),
        ("grandchild.rs", "pub fn made()->i64{911_i64}"),
        ("child/leaf.rs", "pub fn made()->i64{919_i64}"),
    ]);
    assert_eq!(value(&output), "2029");
    compare_nested_physical_native(&root, &output);
}
#[test]
fn cli_nested_adopted_macro_selection_keeps_root_and_parent_cutoffs() {
    let (root, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! choose{()=>{pub fn made()->i64{2039_i64}}}macro_rules! emit{()=>{mod child;}}emit!();macro_rules! choose{()=>{pub fn made()->i64{929_i64}}}fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "macro_rules! emit{()=>{mod grandchild;}}emit!();macro_rules! choose{()=>{pub fn made()->i64{937_i64}}}pub fn made()->i64{grandchild::made()}",
        ),
        ("child/grandchild.rs", "choose!();"),
    ]);
    assert_eq!(value(&output), "2039");
    compare_nested_physical_native(&root, &output);
}

fn compare_nested_physical_native(root: &std::path::Path, cli: &Output) {
    compare_module_native(root, cli);
    let path = root.join("inline-comparison.json");
    let mut report: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    report["scope"] = "Actual CLI nested generated physical module adoption versus isolated native copies; exact parent capture/directory/source-owner and inherited root/parent cutoffs; broader service/quota/fixture/JIT/release pending".into();
    fs::write(
        root.join("nested-physical-comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
