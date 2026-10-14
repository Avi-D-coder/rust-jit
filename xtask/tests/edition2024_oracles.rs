//! Explicit native-only preparation controls. No LRR unit is executed or marked passed.
use lrr::engine::fixture::FixtureManifest;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn native_fixture(ident: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compat/fixtures/edition2024");
    let directory = root.join(ident);
    let fixture =
        FixtureManifest::parse(&fs::read_to_string(directory.join("fixture.toml")).unwrap())
            .unwrap();
    assert!(fixture.oracle);
    assert_eq!(fixture.entry, "case");
    assert_eq!(fixture.source, "main.rs");
    let source = fs::read_to_string(directory.join(&fixture.source)).unwrap();
    assert!(source.len() < 32768);
    assert_eq!(source.matches("#[test]\nfn case()").count(), 1);
    let wrapper = r#"
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(case);
    let result = std::env::args().nth(1).expect("host result path");
    std::fs::write(result, if outcome.is_ok() {"completed"} else {"panicked"}).expect("host result write");
    if outcome.is_err() {std::process::exit(1);}
}
"#;
    let wrapped = source.replacen("#[test]\nfn case()", "fn case()", 1) + wrapper;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp =
        std::env::temp_dir().join(format!("lrr-h002-{}-{ident}-{unique}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(temp.clone());
    let task = if ident == "E2408" { "H003" } else { "H002" };
    let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/macos-first/{}-native-oracles/run-{}-{ident}-{unique}",
        task.to_lowercase(),
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    let oracle = evidence.join("oracle.rs");
    fs::write(&oracle, &wrapped).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(version_text.contains("release: 1.99.0"));
    assert!(version_text.contains(&format!("host: {}", lrr::core::profile::GUEST_TARGET)));
    fs::write(evidence.join("rustc.stdout"), &version.stdout).unwrap();
    let executable = temp.join("oracle");
    let result = temp.join("result.txt");
    let flags = [
        "+1.99.0",
        "--edition=2024",
        "--cfg",
        "test",
        "-C",
        "opt-level=0",
        "-C",
        "overflow-checks=yes",
        "-C",
        "debug-assertions=yes",
        "-C",
        "panic=unwind",
        "--crate-name",
        "lrr_h002_native_oracle",
    ];
    let build = Command::new("rustc")
        .args(flags)
        .arg(&oracle)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    fs::write(evidence.join("build.stdout"), &build.stdout).unwrap();
    fs::write(evidence.join("build.stderr"), &build.stderr).unwrap();
    assert!(
        build.status.success(),
        "{ident}: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&executable).arg(&result).output().unwrap();
    fs::write(evidence.join("guest.stdout"), &run.stdout).unwrap();
    fs::write(evidence.join("guest.stderr"), &run.stderr).unwrap();
    assert!(
        run.status.success(),
        "native {ident}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(fs::read_to_string(result).unwrap(), "completed");
    assert!(run.stdout.len() <= 1_048_576 && run.stderr.len() <= 1_048_576);
    let stdout = String::from_utf8(run.stdout).unwrap();
    let stderr = String::from_utf8(run.stderr).unwrap();
    assert_eq!(stdout, fixture.expect.stdout);
    assert_eq!(
        stdout,
        fs::read_to_string(directory.join("oracle.stdout")).unwrap()
    );
    assert_eq!(stderr, fixture.expect.stderr);
    let before = |a: &str, b: &str| {
        assert!(
            stdout.find(a).unwrap() < stdout.find(b).unwrap(),
            "{ident}: {a} must precede {b}"
        )
    };
    match ident {
        "E2401" => before("drop:scrutinee", "else_entered"),
        "E2402" => {
            before("borrowed:scrutinee", "drop:consequent");
            before("drop:consequent", "drop:scrutinee");
            before("drop:scrutinee", "after_if");
        }
        "E2403" => {
            before("drop:tail", "drop:local");
            before("drop:local", "after:7");
        }
        "E2404" => {
            before("using:", "block_end");
            before("block_end", "drop:struct");
            before("drop:struct", "drop:tuple");
            before("drop:tuple", "drop:direct");
        }
        "E2405" => {
            assert!(!stdout.contains("forbidden_later_call"));
            before("else_entered", "after_if");
            let relaxed = FixtureManifest::parse(
                &fs::read_to_string(directory.join("relaxed-fixture.toml")).unwrap(),
            )
            .unwrap();
            assert!(!relaxed.oracle);
            assert_eq!(relaxed.laziness.no_marker_preparation, ["later_clause"]);
        }
        "E2406" => {
            assert_eq!(stdout.matches("evaluate:first").count(), 1);
            assert_eq!(stdout.matches("evaluate:second").count(), 1);
            before("evaluate:first", "evaluate:second");
            before("drop:second", "drop:first");
            before("drop:first", "else_entered");
        }
        "E2407" => {
            assert_eq!(stdout.matches("drop:iteration0").count(), 1);
            assert_eq!(stdout.matches("drop:iteration1").count(), 1);
            before("continue", "drop:iteration0");
            before("drop:iteration0", "evaluate:1");
            before("break", "drop:iteration1");
            before("drop:iteration1", "after_while");
        }
        "E2408" => {
            assert_eq!(stdout, "explicit:3\ninherited:4\nnested:5\nmutable:7\n");
        }
        "E2410" => {
            before("drop:scrutinee", "else_entered");
            before("after_if", "drop:tail");
            before("drop:tail", "drop:local");
        }
        _ => panic!("unknown fixed native fixture"),
    }
    let record = serde_json::json!({"schema":1,"task":task,"fixture_id":ident,"runner":"native_rust","scope":"native_only","status":"passed","full_lrr_passed":false,"release_complete":false,"platform":lrr::core::profile::GUEST_TARGET,"rust_version":"1.99.0","edition":2024,"cfg_test":true,"flags":flags,"source_blake3":lrr::core::digest::Digest::of_bytes(source.as_bytes()),"stdout":stdout,"stderr":stderr});
    fs::write(
        evidence.join("record.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
}
#[test]
fn native_e2401_failed_iflet_drops_scrutinee_before_else() {
    native_fixture("E2401");
}
#[test]
fn native_e2402_successful_iflet_cleans_locals_before_scrutinee() {
    native_fixture("E2402");
}
#[test]
fn native_e2403_block_tail_temporary_drops_before_local() {
    native_fixture("E2403");
}
#[test]
fn native_e2404_syntactic_let_extension_preserves_referents() {
    native_fixture("E2404");
}
#[test]
fn native_e2405_valid_counterpart_keeps_later_clause_cold() {
    native_fixture("E2405");
}
#[test]
fn native_e2406_failed_chain_cleans_reverse_before_else_once() {
    native_fixture("E2406");
}
#[test]
fn native_e2407_continue_break_retire_distinct_iteration_probes() {
    native_fixture("E2407");
}
#[test]
fn native_e2410_migrated_narrowing_cases_have_recorded_baseline() {
    native_fixture("E2410");
}

#[test]
fn native_e2408_explicit_and_inherited_plain_bindings_have_recorded_baseline() {
    native_fixture("E2408");
}

#[test]
fn native_e2408_inherited_mut_distinguishes_2021_and_2024_without_live_compilation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let source = root.join("compat/fixtures/edition2024/E2408/inherited-mut.rs");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let evidence = root.join(format!(
        "artifacts/macos-first/h003-native-oracles/pair-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    for edition in [2021, 2024] {
        let binary = evidence.join(format!("oracle-{edition}"));
        let build = Command::new("rustc")
            .arg("+1.99.0")
            .arg(format!("--edition={edition}"))
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        fs::write(
            evidence.join(format!("build-{edition}.stdout")),
            &build.stdout,
        )
        .unwrap();
        fs::write(
            evidence.join(format!("build-{edition}.stderr")),
            &build.stderr,
        )
        .unwrap();
        if edition == 2021 {
            assert!(
                build.status.success(),
                "{}",
                String::from_utf8_lossy(&build.stderr)
            );
            let run = Command::new(&binary).output().unwrap();
            assert!(run.status.success());
            assert_eq!(run.stdout, b"legacy:3\n");
            assert!(run.stderr.is_empty());
            fs::write(evidence.join("run-2021.stdout"), &run.stdout).unwrap();
            fs::remove_file(binary).unwrap();
        } else {
            assert_eq!(build.status.code(), Some(1));
            assert!(
                String::from_utf8_lossy(&build.stderr).contains(
                    "cannot mutably bind by value within an implicitly-borrowing pattern"
                )
            );
            assert!(!binary.exists());
        }
    }
    let record = serde_json::json!({"schema":1,"task":"H003","fixture_id":"E2408","runner":"native_rust","scope":"native_only","status":"passed","full_lrr_passed":false,"release_complete":false,"platform":lrr::core::profile::GUEST_TARGET,"rust_version":"1.99.0","editions":[2021,2024],"source_blake3":lrr::core::digest::Digest::of_bytes(&fs::read(source).unwrap())});
    fs::write(
        evidence.join("record.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
}
