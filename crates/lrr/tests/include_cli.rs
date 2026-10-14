use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture(files: &[(&str, &str)]) -> (PathBuf, Output) {
    fixture_with_setup(files, |_| {})
}
fn fixture_with_setup(
    files: &[(&str, &str)],
    setup: impl FnOnce(&std::path::Path),
) -> (PathBuf, Output) {
    let sequence = NEXT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m013-include-cli-{}-{sequence}",
            std::process::id()
        ));
    fs::create_dir_all(&root).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    setup(&root);
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
#[test]
fn cli_literal_include_contributes_the_reached_function() {
    let (_, output) = fixture(&[
        ("main.rs", "include!(\"part.rs\");fn main()->i64{made()}"),
        ("part.rs", "pub fn made()->i64{719_i64}"),
    ]);
    assert_eq!(value(&output), "719");
}
#[test]
fn cli_nested_concat_include_adopts_the_path_before_capture() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}",
        ),
        ("part.rs", "pub fn made()->i64{721_i64}"),
    ]);
    assert_eq!(value(&output), "721");
}
#[test]
fn cli_included_macro_definition_resolves_a_later_producer() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"part.rs\");inner!();fn main()->i64{made()}",
        ),
        (
            "part.rs",
            "macro_rules! inner{()=>{pub fn made()->i64{727_i64}}}",
        ),
    ]);
    assert_eq!(value(&output), "727");
}
#[test]
fn cli_captured_child_include_uses_its_containing_source_directory() {
    let (_, output) = fixture(&[
        ("main.rs", "mod child;fn main()->i64{child::made()}"),
        ("child/mod.rs", "include!(\"part.rs\");"),
        ("child/part.rs", "pub fn made()->i64{733_i64}"),
    ]);
    assert_eq!(value(&output), "733");
}
#[test]
fn cli_cfg_disabled_include_does_not_request_missing_source() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[cfg(any())]include!(\"absent.rs\");fn main()->i64{739_i64}",
    )]);
    assert_eq!(value(&output), "739");
}
#[test]
fn cli_source_include_macro_shadows_the_compiler_export() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! include{($p:expr)=>{fn made()->i64{743_i64}}}include!(\"absent.rs\");fn main()->i64{made()}",
    )]);
    assert_eq!(value(&output), "743");
}
#[test]
fn cli_missing_include_cannot_start_an_entry() {
    let (_, output) = fixture(&[("main.rs", "include!(\"absent.rs\");fn main()->i64{751_i64}")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("LRR-E-SOURCE-IO") || stderr.contains("LRR-E-SOURCE-PATH"),
        "{stderr}"
    );
}
#[test]
fn cli_malformed_include_retains_capture_origins_and_cannot_start_an_entry() {
    let (_, output) = fixture(&[
        ("main.rs", "include!(\"broken.rs\");fn main()->i64{757_i64}"),
        ("broken.rs", "fn broken( {"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("LRR-E-INCLUDE-PARSE"), "{stderr}");
    assert!(
        stderr.contains("broken.rs")
            && stderr.contains("caller origin=")
            && stderr.contains("include origin=")
    );
}
#[test]
fn cli_included_function_values_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    for files in [
        vec![
            ("main.rs", "include!(\"part.rs\");fn main()->i64{made()}"),
            ("part.rs", "fn made()->i64{761_i64}"),
        ],
        vec![
            (
                "main.rs",
                "include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}",
            ),
            ("part.rs", "fn made()->i64{769_i64}"),
        ],
        vec![
            ("main.rs", "mod child;fn main()->i64{child::made()}"),
            ("child/mod.rs", "include!(\"part.rs\");"),
            ("child/part.rs", "pub fn made()->i64{773_i64}"),
        ],
    ] {
        let (root, output) = fixture(&files);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        fs::write(
            root.join("oracle.rs"),
            files[0].1.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){::std::println!(\"{}\",case());}\n",
        )
        .unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
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
        let native = Command::new(root.join("oracle")).output().unwrap();
        fs::write(root.join("native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn cli_included_entry_function_is_selected_after_discovery_seals() {
    let (_, output) = fixture(&[
        ("main.rs", "include!(\"entry.rs\");"),
        ("entry.rs", "fn main()->i64{787_i64}"),
    ]);
    assert_eq!(value(&output), "787");
}
#[test]
fn cli_next_run_recaptures_changed_included_body() {
    let (root, output) = fixture(&[
        ("main.rs", "include!(\"part.rs\");fn main()->i64{made()}"),
        ("part.rs", "fn made()->i64{797_i64}"),
    ]);
    assert_eq!(value(&output), "797");
    fs::write(root.join("part.rs"), "fn made()->i64{809_i64}").unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_lrr"))
        .arg("run")
        .arg(root.join("main.rs"))
        .output()
        .unwrap();
    fs::write(root.join("second.stdout"), &second.stdout).unwrap();
    fs::write(root.join("second.stderr"), &second.stderr).unwrap();
    assert_eq!(value(&second), "809");
}
#[test]
fn cli_include_capture_refuses_a_symlink_outside_the_selected_source_root() {
    let (_, output) = fixture_with_setup(
        &[("main.rs", "include!(\"escape.rs\");fn main()->i64{made()}")],
        |root| {
            let outside = root.with_extension("outside.rs");
            fs::write(&outside, "fn made()->i64{811_i64}").unwrap();
            std::os::unix::fs::symlink(outside, root.join("escape.rs")).unwrap();
        },
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("LRR-E-SOURCE-PATH"), "{stderr}");
}

#[test]
fn cli_core_qualified_include_and_concat_use_compiler_identity() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "core::include!(core::concat!(\"part\",\".rs\"));fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{821_i64}"),
    ]);
    assert_eq!(value(&output), "821");
}
#[test]
fn cli_std_qualified_include_keeps_the_core_definition_owner() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "std::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{823_i64}"),
    ]);
    assert_eq!(value(&output), "823");
}
#[test]
fn cli_named_compiler_macro_import_retains_its_definition() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "use core::include as load;load!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{827_i64}"),
    ]);
    assert_eq!(value(&output), "827");
}
#[test]
fn cli_glob_compiler_macro_import_resolves_before_sealing() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "use core::*;include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{829_i64}"),
    ]);
    assert_eq!(value(&output), "829");
}
#[test]
fn cli_no_std_preserves_core_compiler_macros() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_std]core::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{839_i64}"),
    ]);
    assert_eq!(value(&output), "839");
}
#[test]
fn cli_no_std_does_not_acquire_implicit_std_compiler_paths() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_std]std::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{853_i64}"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-CATALOG-INCOMPLETE")
    );
}
#[test]
fn cli_no_std_explicit_std_declaration_restores_its_macro_export() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_std]extern crate std;std::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{857_i64}"),
    ]);
    assert_eq!(value(&output), "857");
}
#[test]
fn cli_no_implicit_prelude_refuses_a_qualified_implicit_compiler_alias() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]core::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{859_i64}"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-CATALOG-INCOMPLETE")
    );
}
#[test]
fn cli_no_implicit_prelude_explicit_core_alias_restores_compiler_lookup() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]extern crate core as tools;tools::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{863_i64}"),
    ]);
    assert_eq!(value(&output), "863");
}
#[test]
fn cli_local_core_module_shadows_the_implicit_compiler_alias() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[macro_export]macro_rules! own{($p:expr)=>{fn made()->i64{877_i64}}}mod core{pub use crate::own as include;}core::include!(\"absent.rs\");fn main()->i64{made()}",
    )]);
    assert_eq!(value(&output), "877");
}
#[test]
fn cli_absolute_core_compiler_path_bypasses_a_local_core_module() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "mod core{}::core::include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{881_i64}"),
    ]);
    assert_eq!(value(&output), "881");
}

#[test]
fn cli_explicit_core_alias_can_execute_runtime_macros_with_disabled_preludes() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]extern crate core as tools;tools::include!(\"part.rs\");fn main()->i64{tools::assert!(true);made()}",
        ),
        ("part.rs", "fn made()->i64{883_i64}"),
    ]);
    assert_eq!(value(&output), "883");
}
#[test]
fn cli_explicit_std_can_execute_runtime_macros_under_no_std() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_std]extern crate std;std::include!(\"part.rs\");fn main()->i64{std::assert!(true);made()}",
        ),
        ("part.rs", "fn made()->i64{887_i64}"),
    ]);
    assert_eq!(value(&output), "887");
}
#[test]
fn cli_compiler_facade_examples_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    for source in [
        "core::include!(core::concat!(\"part\",\".rs\"));fn main()->i64{made()}",
        "std::include!(\"part.rs\");fn main()->i64{made()}",
        "use core::include as load;load!(\"part.rs\");fn main()->i64{made()}",
        "use core::*;include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}",
        "#![no_std]core::include!(\"part.rs\");fn main()->i64{made()}",
        "#![no_std]extern crate std;std::include!(\"part.rs\");fn main()->i64{std::assert!(true);made()}",
        "#![no_implicit_prelude]extern crate core as tools;tools::include!(\"part.rs\");fn main()->i64{tools::assert!(true);made()}",
        "#[macro_export]macro_rules! own{($p:expr)=>{fn made()->i64{907_i64}}}mod core{pub use crate::own as include;}core::include!(\"absent.rs\");fn main()->i64{made()}",
        "mod core{}::core::include!(\"part.rs\");fn main()->i64{made()}",
    ] {
        let (root, output) =
            fixture(&[("main.rs", source), ("part.rs", "fn made()->i64{911_i64}")]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        let oracle = source.replace("fn main()->i64", "fn case()->i64")
            + "\nextern crate std as oracle_std;fn main(){oracle_std::println!(\"{}\",case());}\n";
        fs::write(root.join("facade_oracle.rs"), oracle).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("facade_oracle.rs"))
            .arg("-o")
            .arg(root.join("facade_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("facade_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("facade_oracle")).output().unwrap();
        fs::write(root.join("facade_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
    for (source, code) in [
        (
            "#![no_std]std::include!(\"part.rs\");fn main()->i64{made()}",
            "E0433",
        ),
        (
            "#![no_implicit_prelude]core::include!(\"part.rs\");fn main()->i64{made()}",
            "E0433",
        ),
        (
            "#![no_implicit_prelude]use core::include as load;load!(\"part.rs\");fn main()->i64{made()}",
            "E0432",
        ),
    ] {
        let (root, output) =
            fixture(&[("main.rs", source), ("part.rs", "fn made()->i64{919_i64}")]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        fs::write(root.join("facade_denied.rs"), source).unwrap();
        let built = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=denied",
                "--crate-type=rlib",
            ])
            .arg(root.join("facade_denied.rs"))
            .arg("-o")
            .arg(root.join("libdenied.rlib"))
            .output()
            .unwrap();
        fs::write(root.join("facade_denied.stderr"), &built.stderr).unwrap();
        assert!(!built.status.success());
        assert!(String::from_utf8(built.stderr).unwrap().contains(code));
    }
}

#[test]
fn cli_no_implicit_prelude_import_requires_an_explicit_compiler_binding() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]use core::include as load;load!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{919_i64}"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-CATALOG-INCOMPLETE")
    );
}
#[test]
fn cli_no_implicit_prelude_import_through_an_explicit_alias_resolves() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]extern crate core as tools;use tools::include as load;load!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{929_i64}"),
    ]);
    assert_eq!(value(&output), "929");
}

#[test]
fn cli_sdk_macro_use_include_and_runtime_exports_share_an_inventory() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#[macro_use(include,assert)]extern crate core;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        ),
        ("part.rs", "fn made()->i64{937_i64}"),
    ]);
    assert_eq!(value(&output), "937");
}
#[test]
fn cli_sdk_macro_use_std_imports_compiler_and_runtime_macros_under_no_std() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_std]#[macro_use]extern crate std;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        ),
        ("part.rs", "fn made()->i64{941_i64}"),
    ]);
    assert_eq!(value(&output), "941");
}
#[test]
fn cli_anonymous_sdk_macro_use_preserves_compiler_identity() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#[macro_use(include,assert)]extern crate core as _;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        ),
        ("part.rs", "fn made()->i64{947_i64}"),
    ]);
    assert_eq!(value(&output), "947");
}
#[test]
fn cli_sdk_selective_macro_use_accepts_unused_supported_exports_without_capture() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[macro_use(env,option_env,concat,include,include_str,include_bytes,assert)]extern crate core;fn main()->i64{assert!(true);953_i64}",
    )]);
    assert_eq!(value(&output), "953");
}
#[test]
fn cli_sdk_macro_use_refuses_an_unknown_export_before_entry() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[macro_use(unavailable)]extern crate core;fn main()->i64{967_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-MACRO-SCOPE")
    );
}
#[test]
fn cli_compiler_globals_survive_a_disabled_macro_use_prelude() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]#[macro_use(include,assert)]extern crate core;include!(\"part.rs\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{971_i64}"),
    ]);
    assert_eq!(value(&output), "971");
}

#[test]
fn cli_sdk_macro_use_selectors_do_not_require_unused_include_files() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[macro_use(include,assert)]extern crate core;mod nested{pub fn made()->i64{assert!(true);977_i64}}fn main()->i64{nested::made()}",
    )]);
    assert_eq!(value(&output), "977");
}
#[test]
fn cli_unused_sdk_option_env_import_does_not_capture_a_variable() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#[macro_use(option_env)]extern crate core;fn main()->i64{983_i64}",
    )]);
    assert_eq!(value(&output), "983");
}
#[test]
fn cli_disabled_macro_use_does_not_supply_non_global_assert_eq() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#![no_implicit_prelude]#[macro_use(assert_eq)]extern crate core;fn main()->i64{assert_eq!(1,1);1013_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
#[test]
fn cli_compiler_globals_work_without_sdk_imports() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]include!(concat!(\"part\",\".rs\"));fn main()->i64{assert!(true);made()}",
        ),
        ("part.rs", "fn made()->i64{1021_i64}"),
    ]);
    assert_eq!(value(&output), "1021");
}
#[test]
fn cli_source_macro_shadows_a_compiler_global_with_prelude_disabled() {
    let (_, output) = fixture(&[(
        "main.rs",
        "#![no_implicit_prelude]macro_rules! include{($p:literal)=>{fn made()->i64{1031_i64}}}include!(\"absent.rs\");fn main()->i64{made()}",
    )]);
    assert_eq!(value(&output), "1031");
}
#[test]
fn cli_sdk_macro_use_examples_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    for source in [
        "#![no_implicit_prelude]#[macro_use(include,assert)]extern crate core;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        "#![no_implicit_prelude]include!(concat!(\"part\",\".rs\"));fn main()->i64{assert!(true);made()}",
        "#![no_implicit_prelude]macro_rules! include{($p:literal)=>{fn made()->i64{1031_i64}}}include!(\"absent.rs\");fn main()->i64{made()}",
        "#[macro_use(include,assert)]extern crate core;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        "#![no_std]#[macro_use]extern crate std;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        "#[macro_use(include,assert)]extern crate core as _;include!(\"part.rs\");fn main()->i64{assert!(true);made()}",
        "#[macro_use(env,option_env,concat,include,include_str,include_bytes,assert)]extern crate core;fn main()->i64{assert!(true);991_i64}",
        "#[macro_use(include,assert)]extern crate core;mod nested{pub fn made()->i64{assert!(true);997_i64}}fn main()->i64{nested::made()}",
    ] {
        let (root, output) =
            fixture(&[("main.rs", source), ("part.rs", "fn made()->i64{1009_i64}")]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        let oracle = source.replace("fn main()->i64", "fn case()->i64")
            + "\nextern crate std as oracle_std;fn main(){oracle_std::println!(\"{}\",case());}\n";
        fs::write(root.join("sdk_oracle.rs"), oracle).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("sdk_oracle.rs"))
            .arg("-o")
            .arg(root.join("sdk_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("sdk_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("sdk_oracle")).output().unwrap();
        fs::write(root.join("sdk_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
    for (source, code) in [
        (
            "#[macro_use(unavailable)]extern crate core;fn main()->i64{1013_i64}",
            "E0469",
        ),
        (
            "#![no_implicit_prelude]#[macro_use(assert_eq)]extern crate core;fn main()->i64{assert_eq!(1,1);1013_i64}",
            "cannot find macro",
        ),
    ] {
        let (root, output) =
            fixture(&[("main.rs", source), ("part.rs", "fn made()->i64{1019_i64}")]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        fs::write(root.join("sdk_denied.rs"), source).unwrap();
        let built = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=rlib",
                "--crate-name=denied",
            ])
            .arg(root.join("sdk_denied.rs"))
            .arg("-o")
            .arg(root.join("libdenied.rlib"))
            .output()
            .unwrap();
        fs::write(root.join("sdk_denied.stderr"), &built.stderr).unwrap();
        assert!(!built.status.success());
        assert!(String::from_utf8(built.stderr).unwrap().contains(code));
    }
}

#[test]
fn cli_include_inside_included_source_resolves_its_containing_directory() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
        ),
        ("nested/first.rs", "include!(\"second.rs\");"),
        ("nested/second.rs", "fn made()->i64{1049_i64}"),
        ("second.rs", "fn made()->i64{1051_i64}"),
    ]);
    assert_eq!(value(&output), "1049");
}
#[test]
fn cli_generated_include_contributes_a_later_function() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{include!(\"part.rs\");}}emit!();fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1061_i64}"),
    ]);
    assert_eq!(value(&output), "1061");
}
#[test]
fn cli_nested_included_concat_preserves_the_containing_source() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
        ),
        ("nested/first.rs", "include!(concat!(\"second\",\".rs\"));"),
        ("nested/second.rs", "fn made()->i64{1063_i64}"),
    ]);
    assert_eq!(value(&output), "1063");
}

#[test]
fn cli_three_included_files_close_before_the_reached_entry() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
        ),
        ("nested/first.rs", "include!(\"deeper/second.rs\");"),
        ("nested/deeper/second.rs", "include!(\"third.rs\");"),
        ("nested/deeper/third.rs", "fn made()->i64{1069_i64}"),
    ]);
    assert_eq!(value(&output), "1069");
}
#[test]
fn cli_generated_concat_include_uses_owned_argument_tokens() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{($p:literal)=>{include!(concat!($p,\".rs\"));}}emit!(\"part\");fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1087_i64}"),
    ]);
    assert_eq!(value(&output), "1087");
}
#[test]
fn cli_included_user_macro_shadows_a_compiler_include_without_capture() {
    let (_, output) = fixture(&[
        ("main.rs", "include!(\"part.rs\");fn main()->i64{made()}"),
        (
            "part.rs",
            "macro_rules! include{($p:literal)=>{fn made()->i64{1091_i64}}}include!(\"absent.rs\");",
        ),
    ]);
    assert_eq!(value(&output), "1091");
}
#[test]
fn cli_nested_include_with_disabled_prelude_retains_compiler_globals() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]include!(\"nested/first.rs\");fn main()->i64{made()}",
        ),
        ("nested/first.rs", "include!(concat!(\"second\",\".rs\"));"),
        ("nested/second.rs", "fn made()->i64{1093_i64}"),
    ]);
    assert_eq!(value(&output), "1093");
}
#[test]
fn cli_missing_nested_include_refuses_before_any_entry_report() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{1097_i64}",
        ),
        ("nested/first.rs", "include!(\"absent.rs\");"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("LRR-E-SOURCE-PATH")
            && stderr.contains("nested/first.rs")
            && stderr.contains("absent.rs"),
        "{stderr}"
    );
}

#[test]
fn cli_malformed_nested_include_retains_its_actual_source_path() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{1103_i64}",
        ),
        ("nested/first.rs", "include!(\"bad.rs\");"),
        ("nested/bad.rs", "fn made( {"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("LRR-E-INCLUDE-PARSE") && stderr.contains("nested/bad.rs"),
        "{stderr}"
    );
}
#[test]
fn cli_nested_include_values_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    for (source, first, second) in [
        (
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
            "include!(\"second.rs\");",
            "fn made()->i64{1117_i64}",
        ),
        (
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
            "include!(concat!(\"second\",\".rs\"));",
            "fn made()->i64{1123_i64}",
        ),
        (
            "#![no_implicit_prelude]include!(\"nested/first.rs\");fn main()->i64{made()}",
            "include!(concat!(\"second\",\".rs\"));",
            "fn made()->i64{1129_i64}",
        ),
        (
            "macro_rules! emit{()=>{include!(\"nested/first.rs\");}}emit!();fn main()->i64{made()}",
            "include!(\"second.rs\");",
            "fn made()->i64{1151_i64}",
        ),
        (
            "macro_rules! emit{($p:literal)=>{include!(concat!($p,\".rs\"));}}emit!(\"nested/first\");fn main()->i64{made()}",
            "include!(concat!(\"second\",\".rs\"));",
            "fn made()->i64{1153_i64}",
        ),
        (
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
            "macro_rules! include{($p:literal)=>{fn made()->i64{1163_i64}}}include!(\"absent.rs\");",
            "fn unused()->i64{1171_i64}",
        ),
    ] {
        let (root, output) = fixture(&[
            ("main.rs", source),
            ("nested/first.rs", first),
            ("nested/second.rs", second),
            ("second.rs", "fn made()->i64{1181_i64}"),
        ]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        fs::write(root.join("nested_oracle.rs"),source.replace("fn main()->i64","fn case()->i64")+"\nextern crate std as oracle_std;fn main(){oracle_std::println!(\"{}\",case());}").unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("nested_oracle.rs"))
            .arg("-o")
            .arg(root.join("nested_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("nested_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("nested_oracle")).output().unwrap();
        fs::write(root.join("nested_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn cli_nested_include_cannot_follow_a_symlink_outside_source_authority() {
    let (_, output) = fixture_with_setup(
        &[
            (
                "main.rs",
                "include!(\"nested/first.rs\");fn main()->i64{made()}",
            ),
            ("nested/first.rs", "include!(\"escape.rs\");"),
        ],
        |root| {
            let outside = root.with_extension("nested-outside.rs");
            fs::write(&outside, "fn made()->i64{1201_i64}").unwrap();
            std::os::unix::fs::symlink(outside, root.join("nested/escape.rs")).unwrap();
        },
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("LRR-E-SOURCE-PATH") && stderr.contains("nested/first.rs"),
        "{stderr}"
    );
}
#[test]
fn cli_included_user_concat_expands_before_capture() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
        ),
        (
            "nested/first.rs",
            "macro_rules! concat{()=>{\"second.rs\"}}include!(concat!());",
        ),
        ("nested/second.rs", "fn made()->i64{1213_i64}"),
    ]);
    assert_eq!(value(&output), "1213");
}

#[test]
fn cli_macro_defined_in_included_source_emits_a_relative_include() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/macros.rs\");emit!();fn main()->i64{made()}",
        ),
        (
            "nested/macros.rs",
            "macro_rules! emit{()=>{include!(\"second.rs\");}}",
        ),
        ("nested/second.rs", "fn made()->i64{1217_i64}"),
        ("second.rs", "fn made()->i64{1223_i64}"),
    ]);
    assert_eq!(value(&output), "1223");
}
#[test]
fn cli_included_macro_with_concat_uses_invocation_file_provenance() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/macros.rs\");emit!();fn main()->i64{made()}",
        ),
        (
            "nested/macros.rs",
            "macro_rules! emit{()=>{include!(concat!(\"second\",\".rs\"));}}",
        ),
        ("nested/second.rs", "fn made()->i64{1229_i64}"),
        ("second.rs", "fn made()->i64{1231_i64}"),
    ]);
    assert_eq!(value(&output), "1231");
}

#[test]
fn cli_macro_invoked_inside_included_source_keeps_that_source_base() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"nested/first.rs\");fn main()->i64{made()}",
        ),
        (
            "nested/first.rs",
            "macro_rules! emit{()=>{include!(\"second.rs\");}}emit!();",
        ),
        ("nested/second.rs", "fn made()->i64{1237_i64}"),
        ("second.rs", "fn made()->i64{1249_i64}"),
    ]);
    assert_eq!(value(&output), "1237");
}
#[test]
fn cli_included_macro_called_from_another_include_uses_the_calling_file() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"definitions/macros.rs\");include!(\"calls/use.rs\");fn main()->i64{made()}",
        ),
        (
            "definitions/macros.rs",
            "macro_rules! emit{()=>{include!(\"second.rs\");}}",
        ),
        ("calls/use.rs", "emit!();"),
        ("calls/second.rs", "fn made()->i64{1259_i64}"),
        ("definitions/second.rs", "fn made()->i64{1277_i64}"),
        ("second.rs", "fn made()->i64{1279_i64}"),
    ]);
    assert_eq!(value(&output), "1259");
}
#[test]
fn cli_included_macro_substituted_path_keeps_mixed_token_sources() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"definitions/macros.rs\");emit!(\"second\");fn main()->i64{made()}",
        ),
        (
            "definitions/macros.rs",
            "macro_rules! emit{($p:literal)=>{include!(concat!($p,\".rs\"));}}",
        ),
        ("second.rs", "fn made()->i64{1283_i64}"),
        ("definitions/second.rs", "fn made()->i64{1289_i64}"),
    ]);
    assert_eq!(value(&output), "1283");
}
#[test]
fn cli_included_macro_source_bases_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    for (source, definition, call) in [
        (
            "include!(\"definitions/macros.rs\");emit!();fn main()->i64{made()}",
            "macro_rules! emit{()=>{include!(\"second.rs\");}}",
            "",
        ),
        (
            "include!(\"definitions/macros.rs\");emit!();fn main()->i64{made()}",
            "macro_rules! emit{()=>{include!(concat!(\"second\",\".rs\"));}}",
            "",
        ),
        (
            "include!(\"definitions/macros.rs\");emit!(\"second\");fn main()->i64{made()}",
            "macro_rules! emit{($p:literal)=>{include!(concat!($p,\".rs\"));}}",
            "",
        ),
        (
            "include!(\"definitions/macros.rs\");include!(\"calls/use.rs\");fn main()->i64{made()}",
            "macro_rules! emit{()=>{include!(\"second.rs\");}}",
            "emit!();",
        ),
        (
            "include!(\"definitions/macros.rs\");include!(\"calls/use.rs\");fn main()->i64{made()}",
            "macro_rules! emit{($p:literal)=>{include!(concat!($p,\".rs\"));}}",
            "emit!(\"second\");",
        ),
    ] {
        let (root, output) = fixture(&[
            ("main.rs", source),
            ("definitions/macros.rs", definition),
            ("calls/use.rs", call),
            ("second.rs", "fn made()->i64{1291_i64}"),
            ("calls/second.rs", "fn made()->i64{1297_i64}"),
            ("definitions/second.rs", "fn made()->i64{1301_i64}"),
        ]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        fs::write(
            root.join("base_oracle.rs"),
            source.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){println!(\"{}\",case());}",
        )
        .unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("base_oracle.rs"))
            .arg("-o")
            .arg(root.join("base_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("base_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("base_oracle")).output().unwrap();
        fs::write(root.join("base_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn cli_included_macro_emitted_include_cannot_escape_source_authority() {
    let (_, output) = fixture_with_setup(
        &[
            (
                "main.rs",
                "include!(\"definitions/macros.rs\");emit!();fn main()->i64{made()}",
            ),
            (
                "definitions/macros.rs",
                "macro_rules! emit{()=>{include!(\"escape.rs\");}}",
            ),
        ],
        |root| {
            let outside = root.with_extension("macro-outside.rs");
            fs::write(&outside, "fn made()->i64{1327_i64}").unwrap();
            std::os::unix::fs::symlink(outside, root.join("escape.rs")).unwrap();
        },
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("LRR-E-SOURCE-PATH")
            && stderr.contains("main.rs")
            && stderr.contains("escape.rs"),
        "{stderr}"
    );
}

#[test]
fn cli_user_path_macro_expands_before_include_capture() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! path{()=>{\"part.rs\"}}include!(path!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1361_i64}"),
    ]);
    assert_eq!(value(&output), "1361");
}
#[test]
fn cli_user_string_macro_is_an_operand_of_compiler_concat() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! stem{()=>{\"part\"}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1367_i64}"),
    ]);
    assert_eq!(value(&output), "1367");
}
#[test]
fn cli_generated_local_path_macro_expands_in_its_own_lexical_scope() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! path{()=>{\"part.rs\"}}include!(path!());}}emit!();fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1373_i64}"),
    ]);
    assert_eq!(value(&output), "1373");
}

#[test]
fn cli_renamed_user_path_macro_preserves_definition_identity() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! location{()=>{\"part.rs\"}}use location as selected;include!(selected!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1381_i64}"),
    ]);
    assert_eq!(value(&output), "1381");
}
#[test]
fn cli_user_path_macro_invalid_string_output_refuses_before_capture() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! path{()=>{42_i64}}include!(path!());fn main()->i64{1399_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("LRR-E-INCLUDE-ARGUMENTS") || stderr.contains("LRR-E-COMPILE-INPUT"),
        "{stderr}"
    );
}
#[test]
fn cli_user_path_macro_cannot_grant_source_authority() {
    let (_, output) = fixture_with_setup(
        &[(
            "main.rs",
            "macro_rules! path{()=>{\"escape.rs\"}}include!(path!());fn main()->i64{made()}",
        )],
        |root| {
            let outside = root.with_extension("user-path-outside.rs");
            fs::write(&outside, "fn made()->i64{1409_i64}").unwrap();
            std::os::unix::fs::symlink(outside, root.join("escape.rs")).unwrap();
        },
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("LRR-E-SOURCE-PATH"), "{stderr}");
}
#[test]
fn cli_user_path_macro_examples_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    for source in [
        "macro_rules! path{()=>{\"part.rs\"}}include!(path!());fn main()->i64{made()}",
        "macro_rules! stem{()=>{\"part\"}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
        "macro_rules! location{()=>{\"part.rs\"}}use location as selected;include!(selected!());fn main()->i64{made()}",
        "macro_rules! emit{()=>{macro_rules! path{()=>{\"part.rs\"}}include!(path!());}}emit!();fn main()->i64{made()}",
        "macro_rules! concat{()=>{\"part.rs\"}}include!(concat!());fn main()->i64{made()}",
    ] {
        let (root, output) =
            fixture(&[("main.rs", source), ("part.rs", "fn made()->i64{1423_i64}")]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        fs::write(
            root.join("path_oracle.rs"),
            source.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){println!(\"{}\",case());}",
        )
        .unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("path_oracle.rs"))
            .arg("-o")
            .arg(root.join("path_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("path_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("path_oracle")).output().unwrap();
        fs::write(root.join("path_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn cli_unqualified_user_macro_import_refuses_an_inert_attribute_collision() {
    for name in [
        "path",
        "allow",
        "no_std",
        "repr",
        "inline",
        "deprecated",
        "cfg",
    ] {
        let source = format!(
            "macro_rules! {name}{{()=>{{\"part.rs\"}}}}use {name} as chosen;include!(chosen!());fn main()->i64{{made()}}"
        );
        let (_, output) = fixture(&[
            ("main.rs", &source),
            ("part.rs", "fn made()->i64{1427_i64}"),
        ]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("LRR-E-MACRO-AMBIGUOUS")
        );
    }
}
#[test]
fn cli_inert_attribute_collision_survives_disabled_implicit_prelude() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#![no_implicit_prelude]macro_rules! path{()=>{\"part.rs\"}}use path as chosen;include!(chosen!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1429_i64}"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-MACRO-AMBIGUOUS")
    );
}
#[test]
fn cli_bare_user_macro_can_shadow_an_inert_attribute_name() {
    for name in ["path", "allow", "repr", "inline", "deprecated"] {
        let source = format!(
            "macro_rules! {name}{{()=>{{\"part.rs\"}}}}include!({name}!());fn main()->i64{{made()}}"
        );
        let (_, output) = fixture(&[
            ("main.rs", &source),
            ("part.rs", "fn made()->i64{1433_i64}"),
        ]);
        assert_eq!(value(&output), "1433");
    }
}
#[test]
fn cli_shadowable_builtin_attribute_macro_names_can_be_imported() {
    for name in ["test", "derive"] {
        let source = format!(
            "macro_rules! {name}{{()=>{{\"part.rs\"}}}}use {name} as chosen;include!(chosen!());fn main()->i64{{made()}}"
        );
        let (_, output) = fixture(&[
            ("main.rs", &source),
            ("part.rs", "fn made()->i64{1439_i64}"),
        ]);
        assert_eq!(value(&output), "1439");
    }
}
#[test]
fn cli_qualified_exported_macro_import_bypasses_inert_attribute_ambiguity() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "#[macro_export]macro_rules! path{()=>{\"part.rs\"}}use crate::path as chosen;include!(chosen!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1447_i64}"),
    ]);
    assert_eq!(value(&output), "1447");
}

#[test]
fn cli_generated_macro_import_refuses_an_inert_attribute_collision() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! path{()=>{\"part.rs\"}}use path as chosen;include!(chosen!());}}emit!();fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1451_i64}"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-MACRO-AMBIGUOUS")
    );
}
#[test]
fn cli_cfg_disabled_import_does_not_create_inert_attribute_ambiguity() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! path{()=>{\"part.rs\"}}#[cfg(any())]use path as chosen;include!(path!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1453_i64}"),
    ]);
    assert_eq!(value(&output), "1453");
}
#[test]
fn cli_inert_attribute_import_cases_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    let mut sources = Vec::new();
    for name in [
        "path",
        "allow",
        "no_std",
        "repr",
        "inline",
        "deprecated",
        "cfg",
    ] {
        sources.push((format!("macro_rules! {name}{{()=>{{\"part.rs\"}}}}use {name} as chosen;include!(chosen!());fn main()->i64{{made()}}"),false));
    }
    sources.push(("#![no_implicit_prelude]macro_rules! path{()=>{\"part.rs\"}}use path as chosen;include!(chosen!());fn main()->i64{made()}".into(),false));
    sources.push(("macro_rules! emit{()=>{macro_rules! path{()=>{\"part.rs\"}}use path as chosen;include!(chosen!());}}emit!();fn main()->i64{made()}".into(),false));
    for name in ["path", "allow", "repr", "inline", "deprecated"] {
        sources.push((format!("macro_rules! {name}{{()=>{{\"part.rs\"}}}}include!({name}!());fn main()->i64{{made()}}"),true));
    }
    for name in ["test", "derive"] {
        sources.push((format!("macro_rules! {name}{{()=>{{\"part.rs\"}}}}use {name} as chosen;include!(chosen!());fn main()->i64{{made()}}"),true));
    }
    sources.push(("#[macro_export]macro_rules! path{()=>{\"part.rs\"}}use crate::path as chosen;include!(chosen!());fn main()->i64{made()}".into(),true));
    sources.push(("macro_rules! path{()=>{\"part.rs\"}}#[cfg(any())]use path as chosen;include!(path!());fn main()->i64{made()}".into(),true));
    for (source, accepted) in sources {
        let (root, output) = fixture(&[
            ("main.rs", &source),
            ("part.rs", "fn made()->i64{1459_i64}"),
        ]);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        let oracle = source.replace("fn main()->i64", "fn case()->i64")
            + "\nextern crate std as oracle_std;fn main(){oracle_std::println!(\"{}\",case());}";
        fs::write(root.join("attr_oracle.rs"), oracle).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("attr_oracle.rs"))
            .arg("-o")
            .arg(root.join("attr_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("attr_compile.stderr"), &built.stderr).unwrap();
        if accepted {
            let actual = value(&output);
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            let native = Command::new(root.join("attr_oracle")).output().unwrap();
            fs::write(root.join("attr_native.stdout"), &native.stdout).unwrap();
            assert!(native.status.success());
            assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
        } else {
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8(output.stderr)
                    .unwrap()
                    .contains("LRR-E-MACRO-AMBIGUOUS")
            );
            assert!(!built.status.success());
            assert!(String::from_utf8(built.stderr).unwrap().contains("E0659"));
        }
    }
}

#[test]
fn cli_user_path_macro_can_return_a_compiler_concat_expression() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! path{()=>{concat!(\"part\",\".rs\")}}include!(path!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1471_i64}"),
    ]);
    assert_eq!(value(&output), "1471");
}
#[test]
fn cli_user_path_macro_can_return_another_user_macro_call() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! leaf{()=>{\"part.rs\"}}macro_rules! path{()=>{leaf!()}}include!(path!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1481_i64}"),
    ]);
    assert_eq!(value(&output), "1481");
}

#[test]
fn cli_recursive_string_chain_preserves_invocation_textual_position() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! leaf{()=>{\"wrong.rs\"}}macro_rules! path{()=>{leaf!()}}macro_rules! leaf{()=>{\"part.rs\"}}include!(path!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1493_i64}"),
    ]);
    assert_eq!(value(&output), "1493");
}
#[test]
fn cli_recursive_string_chain_in_generated_include_uses_generated_scope() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! outer{()=>{macro_rules! leaf{()=>{\"part.rs\"}}macro_rules! path{()=>{leaf!()}}include!(path!());}}outer!();fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1499_i64}"),
    ]);
    assert_eq!(value(&output), "1499");
}
#[test]
fn cli_recursive_string_chain_uses_the_invocation_file_directory() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "include!(\"defs/macros.rs\");include!(path!());fn main()->i64{made()}",
        ),
        (
            "defs/macros.rs",
            "macro_rules! leaf{()=>{concat!(\"part\",\".rs\")}}macro_rules! path{()=>{leaf!()}}",
        ),
        ("part.rs", "fn made()->i64{1511_i64}"),
        ("defs/part.rs", "fn made()->i64{1_i64}"),
    ]);
    assert_eq!(value(&output), "1511");
}
#[test]
fn cli_recursive_string_chain_refuses_an_absent_returned_macro() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! path{()=>{absent!()}}include!(path!());fn main()->i64{0_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("LRR-E-"));
}
#[test]
fn cli_recursive_string_chain_refuses_a_non_string_output() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! leaf{()=>{7_i64}}macro_rules! path{()=>{leaf!()}}include!(path!());fn main()->i64{0_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("LRR-E-COMPILE-INPUT: compile-time string operand requires a string literal")
    );
}
#[test]
fn cli_recursive_string_chain_uses_a_returned_user_concat_definition() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! concat{($($t:tt)*)=>{\"part.rs\"}}macro_rules! path{()=>{concat!(\"wrong.rs\")}}include!(path!());fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1523_i64}"),
    ]);
    assert_eq!(value(&output), "1523");
}
#[test]
fn cli_recursive_string_chains_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    let sources = [
        "macro_rules! path{()=>{concat!(\"part\",\".rs\")}}include!(path!());fn main()->i64{made()}",
        "macro_rules! leaf{()=>{\"part.rs\"}}macro_rules! path{()=>{leaf!()}}include!(path!());fn main()->i64{made()}",
        "macro_rules! leaf{()=>{\"wrong.rs\"}}macro_rules! path{()=>{leaf!()}}macro_rules! leaf{()=>{\"part.rs\"}}include!(path!());fn main()->i64{made()}",
        "macro_rules! outer{()=>{macro_rules! leaf{()=>{\"part.rs\"}}macro_rules! path{()=>{leaf!()}}include!(path!());}}outer!();fn main()->i64{made()}",
        "macro_rules! concat{($($t:tt)*)=>{\"part.rs\"}}macro_rules! path{()=>{concat!(\"wrong.rs\")}}include!(path!());fn main()->i64{made()}",
    ];
    for source in sources {
        let (root, output) =
            fixture(&[("main.rs", source), ("part.rs", "fn made()->i64{1531_i64}")]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        let oracle = source.replace("fn main()->i64", "fn case()->i64")
            + "\nfn main(){println!(\"{}\",case());}";
        fs::write(root.join("chain_oracle.rs"), oracle).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("chain_oracle.rs"))
            .arg("-o")
            .arg(root.join("chain_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("chain_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("chain_oracle")).output().unwrap();
        fs::write(root.join("chain_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn cli_concat_operand_resumes_a_returned_user_macro_call() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! leaf{()=>{\"part\"}}macro_rules! stem{()=>{leaf!()}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1543_i64}"),
    ]);
    assert_eq!(value(&output), "1543");
}
#[test]
fn cli_concat_operand_resumes_a_returned_compiler_macro_call() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! stem{()=>{concat!(\"pa\",\"rt\")}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1549_i64}"),
    ]);
    assert_eq!(value(&output), "1549");
}

#[test]
fn cli_recursive_concat_operand_accepts_a_returned_numeric_literal() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! leaf{()=>{7}}macro_rules! number{()=>{leaf!()}}include!(concat!(\"part\",number!(),\".rs\"));fn main()->i64{made()}",
        ),
        ("part7.rs", "fn made()->i64{1553_i64}"),
    ]);
    assert_eq!(value(&output), "1553");
}
#[test]
fn cli_recursive_concat_operands_in_generated_source_keep_their_scope() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{macro_rules! leaf{()=>{\"part\"}}macro_rules! stem{()=>{leaf!()}}include!(concat!(stem!(),\".rs\"));}}emit!();fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1559_i64}"),
    ]);
    assert_eq!(value(&output), "1559");
}
#[test]
fn cli_recursive_concat_operands_keep_independent_producer_chains() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! leaf{($s:literal)=>{$s}}macro_rules! stem{()=>{leaf!(\"part\")}}macro_rules! suffix{()=>{leaf!(\".rs\")}}include!(concat!(stem!(),suffix!()));fn main()->i64{made()}",
        ),
        ("part.rs", "fn made()->i64{1567_i64}"),
    ]);
    assert_eq!(value(&output), "1567");
}
#[test]
fn cli_recursive_concat_operand_refuses_parent_argument_injection() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! leaf{()=>{\"part\",\".rs\"}}macro_rules! stem{()=>{leaf!()}}include!(concat!(stem!()));fn main()->i64{0_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("exactly one expression"), "{stderr}");
}
#[test]
fn cli_recursive_concat_operands_match_native_macos() {
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    let sources = [
        "macro_rules! leaf{()=>{\"part\"}}macro_rules! stem{()=>{leaf!()}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
        "macro_rules! stem{()=>{concat!(\"pa\",\"rt\")}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
        "macro_rules! leaf{()=>{7}}macro_rules! number{()=>{leaf!()}}include!(concat!(\"part\",number!(),\".rs\"));fn main()->i64{made()}",
        "macro_rules! emit{()=>{macro_rules! leaf{()=>{\"part\"}}macro_rules! stem{()=>{leaf!()}}include!(concat!(stem!(),\".rs\"));}}emit!();fn main()->i64{made()}",
        "macro_rules! leaf{($s:literal)=>{$s}}macro_rules! stem{()=>{leaf!(\"part\")}}macro_rules! suffix{()=>{leaf!(\".rs\")}}include!(concat!(stem!(),suffix!()));fn main()->i64{made()}",
    ];
    for source in sources {
        let (root, output) = fixture(&[
            ("main.rs", source),
            ("part.rs", "fn made()->i64{1571_i64}"),
            ("part7.rs", "fn made()->i64{1571_i64}"),
        ]);
        let actual = value(&output);
        fs::write(root.join("rustc.stdout"), &version.stdout).unwrap();
        let oracle = source.replace("fn main()->i64", "fn case()->i64")
            + "\nfn main(){println!(\"{}\",case());}";
        fs::write(root.join("operand_oracle.rs"), oracle).unwrap();
        let built = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
            .arg(root.join("operand_oracle.rs"))
            .arg("-o")
            .arg(root.join("operand_oracle"))
            .output()
            .unwrap();
        fs::write(root.join("operand_compile.stderr"), &built.stderr).unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let native = Command::new(root.join("operand_oracle")).output().unwrap();
        fs::write(root.join("operand_native.stdout"), &native.stdout).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn cli_recursive_environment_operand_refuses_a_numeric_name_before_capture() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! leaf{()=>{7}}macro_rules! name{()=>{leaf!()}}include!(env!(name!()));fn main()->i64{0_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("requires a string literal"), "{stderr}");
}
#[test]
fn cli_recursive_concat_operand_refuses_an_absent_returned_macro() {
    let (_, output) = fixture(&[(
        "main.rs",
        "macro_rules! stem{()=>{absent!()}}include!(concat!(stem!(),\".rs\"));fn main()->i64{0_i64}",
    )]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("LRR-E-CATALOG-INCOMPLETE: nested compile input requires complete source macro resolution"),"{stderr}");
}

#[test]
fn cli_generated_external_module_never_seals_as_an_empty_module() {
    let (_, output) = fixture(&[
        (
            "main.rs",
            "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}",
        ),
        (
            "child.rs",
            "pub fn made()->i64{1583_i64}pub fn cold(){missing();}",
        ),
    ]);
    assert_eq!(
        value(&output),
        "1583",
        "the actual child function must be present and executable while its cold body stays unentered"
    );
}
