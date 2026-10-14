use lrr::engine::fixture::{
    FixtureManifest, FixtureMode, FixtureObservation, FixtureOutcome, MarkerObservation,
};
const SIMPLE: &str = r#"schema_version = 1
id = "T034-CONTRACT"
category = "relaxed"
source = "main.rs"
entry = "case"
modes = ["off", "auto", "force"]
oracle = false
[expect]
status = "passed"
stdout = ""
stderr = ""
[laziness]
no_body_preparation = ["broken"]
no_marker_preparation = ["cold_branch"]
[force]
minimum_native_entries = 0
minimum_native_scalar_ops = 0
"#;
fn observed(mode: FixtureMode) -> FixtureObservation {
    FixtureObservation {
        outcome: Some(FixtureOutcome::Completed),
        mode: Some(mode),
        status: Some("passed".into()),
        stdout: Some(String::new()),
        stderr: Some(String::new()),
        body_preparations: std::collections::BTreeMap::from([("broken".into(), 0)]),
        markers: std::collections::BTreeMap::from([(
            "cold_branch".into(),
            MarkerObservation {
                thunk_executions: Some(0),
                installed_operations: Some(0),
                native_operations: Some(0),
            },
        )]),
        native_entries: Some(0),
        native_scalar_ops: Some(0),
        ..Default::default()
    }
}
#[test]
fn fixture_contract_parses_normative_disk_shape_and_rejects_unknown_fields_at_every_level() {
    let fixture = FixtureManifest::parse(SIMPLE).unwrap();
    assert_eq!(fixture.id, "T034-CONTRACT");
    assert_eq!(fixture.source, "main.rs");
    assert_eq!(fixture.modes.len(), 3);
    assert!(
        FixtureManifest::parse(
            &SIMPLE
                .replace("category = \"relaxed\"", "category = \"strict\"")
                .replace("oracle = false", "oracle = true")
        )
        .is_ok()
    );
    assert!(FixtureManifest::parse(&SIMPLE.replace("oracle = false", "oracle = true")).is_err());
    assert!(FixtureManifest::parse(&SIMPLE.replace("\"force\"", "\"turbo\"")).is_err());
    for text in [
        format!("unknown = true\n{SIMPLE}"),
        SIMPLE.replace("status =", "unknown = true\nstatus ="),
        SIMPLE.replace(
            "no_body_preparation =",
            "unknown = true\nno_body_preparation =",
        ),
        SIMPLE.replace(
            "minimum_native_entries =",
            "unknown = true\nminimum_native_entries =",
        ),
    ] {
        assert_eq!(
            FixtureManifest::parse(&text).unwrap_err().code,
            "LRR-E-FIXTURE"
        );
    }
    for text in [
        SIMPLE.replace("schema_version = 1", "schema_version = 2"),
        SIMPLE.replace("main.rs", "../main.rs"),
        SIMPLE.replace("main.rs", "/tmp/main.rs"),
        SIMPLE.replace("main.rs", "a//main.rs"),
        SIMPLE.replace("[\"off\", \"auto\", \"force\"]", "[\"off\", \"off\"]"),
        SIMPLE.replace("[\"off\", \"auto\", \"force\"]", "[]"),
        SIMPLE
            .replace("oracle = false", "oracle = true")
            .replace("entry = \"case\"", "entry = \"nested::case\""),
    ] {
        assert!(FixtureManifest::parse(&text).is_err());
    }
}
#[test]
fn fixture_contract_requires_explicit_zero_observations_and_all_marker_channels() {
    let fixture = FixtureManifest::parse(SIMPLE).unwrap();
    for mode in [FixtureMode::Off, FixtureMode::Auto, FixtureMode::Force] {
        fixture.check(mode, &observed(mode)).unwrap();
    }
    for field in 0..10 {
        let mut report = observed(FixtureMode::Force);
        match field {
            0 => report.mode = None,
            1 => report.status = None,
            2 => report.stdout = None,
            3 => report.stderr = None,
            4 => {
                report.body_preparations.clear();
            }
            5 => {
                report
                    .markers
                    .get_mut("cold_branch")
                    .unwrap()
                    .thunk_executions = None
            }
            6 => {
                report
                    .markers
                    .get_mut("cold_branch")
                    .unwrap()
                    .installed_operations = None
            }
            7 => {
                report
                    .markers
                    .get_mut("cold_branch")
                    .unwrap()
                    .native_operations = None
            }
            8 => report.native_entries = None,
            9 => report.outcome = None,
            _ => unreachable!(),
        }
        assert!(
            fixture.check(FixtureMode::Force, &report).is_err(),
            "unobserved field {field} accepted"
        );
    }
    let mut report = observed(FixtureMode::Force);
    report.native_scalar_ops = None;
    assert!(fixture.check(FixtureMode::Force, &report).is_err());
    let mut report = observed(FixtureMode::Off);
    report
        .markers
        .get_mut("cold_branch")
        .unwrap()
        .installed_operations = Some(1);
    assert!(fixture.check(FixtureMode::Off, &report).is_err());
    let mut report = observed(FixtureMode::Off);
    report.body_preparations.insert("broken".into(), 1);
    assert!(fixture.check(FixtureMode::Off, &report).is_err());
}
#[test]
fn fixture_contract_preserves_exact_output_ordered_duplicate_drops_and_fault_cleanup() {
    let text=SIMPLE.replace("status = \"passed\"","status = \"fault\"\ndiagnostic_code = \"LRR-E-UNSUPPORTED\"\nphase = \"execution\"\ncleanup_ran = false\nexpected_drop_log = [\"same\", \"same\", \"last\"]").replace("stdout = \"\"","stdout = \"written\\n\"");
    let fixture = FixtureManifest::parse(&text).unwrap();
    let mut report = observed(FixtureMode::Off);
    report.outcome = Some(FixtureOutcome::Fault);
    report.status = Some("fault".into());
    report.stdout = Some("written\n".into());
    report.diagnostic_code = Some("LRR-E-UNSUPPORTED".into());
    report.phase = Some("execution".into());
    report.cleanup_ran = Some(false);
    report.drop_log = Some(vec!["same".into(), "same".into(), "last".into()]);
    fixture.check(FixtureMode::Off, &report).unwrap();
    report.cleanup_ran = None;
    assert!(fixture.check(FixtureMode::Off, &report).is_err());
    report.cleanup_ran = Some(true);
    assert!(fixture.check(FixtureMode::Off, &report).is_err());
    report.cleanup_ran = Some(false);
    report.drop_log = Some(vec!["last".into(), "same".into(), "same".into()]);
    assert!(fixture.check(FixtureMode::Off, &report).is_err());
    report.drop_log = Some(vec!["same".into(), "same".into(), "last".into()]);
    report.stdout = Some("written".into());
    assert!(fixture.check(FixtureMode::Off, &report).is_err());
}
#[test]
fn fixture_contract_panic_never_accepts_fault_or_missing_message_and_force_checks_real_minima() {
    let text = SIMPLE
        .replace(
            "status = \"passed\"",
            "status = \"panicked\"\npanic_contains = \"actual message\"",
        )
        .replace("minimum_native_entries = 0", "minimum_native_entries = 2")
        .replace(
            "minimum_native_scalar_ops = 0",
            "minimum_native_scalar_ops = 7",
        );
    let fixture = FixtureManifest::parse(&text).unwrap();
    let mut report = observed(FixtureMode::Force);
    report.outcome = Some(FixtureOutcome::GuestPanic);
    report.status = Some("panicked".into());
    report.panic_message = Some("prefix actual message suffix".into());
    report.native_entries = Some(2);
    report.native_scalar_ops = Some(7);
    fixture.check(FixtureMode::Force, &report).unwrap();
    report.diagnostic_code = Some("LRR-E-UNSUPPORTED".into());
    assert!(fixture.check(FixtureMode::Force, &report).is_err());
    report.diagnostic_code = None;
    report.panic_message = None;
    assert!(fixture.check(FixtureMode::Force, &report).is_err());
    report.panic_message = Some("actual message".into());
    report.native_entries = Some(1);
    assert!(fixture.check(FixtureMode::Force, &report).is_err());
    assert!(
        FixtureManifest::parse(&text.replace(
            "panic_contains =",
            "diagnostic_code = \"LRR-E-UNSUPPORTED\"\npanic_contains ="
        ))
        .is_err()
    );
}

#[test]
fn fixture_catalog_reads_disk_contract_and_refuses_duplicate_ids_and_unbounded_metadata() {
    use lrr::engine::fixture::FixtureCatalog;
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../compat/fixtures/T034-FORMAT");
    let text = std::fs::read_to_string(directory.join("fixture.toml")).unwrap();
    let fixture = FixtureManifest::parse(&text).unwrap();
    assert!(directory.join(&fixture.source).is_file());
    let catalog =
        FixtureCatalog::new(vec![fixture, FixtureManifest::parse(SIMPLE).unwrap()]).unwrap();
    assert_eq!(
        catalog
            .manifests()
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        ["T034-CONTRACT", "T034-CONTRACT-ON-DISK"]
    );
    assert!(catalog.get("missing").is_none());
    assert!(
        FixtureCatalog::new(vec![
            FixtureManifest::parse(SIMPLE).unwrap(),
            FixtureManifest::parse(SIMPLE).unwrap()
        ])
        .is_err()
    );
    let mut fixture = FixtureManifest::parse(SIMPLE).unwrap();
    fixture.laziness.no_body_preparation = (0..10_000)
        .map(|i| format!("{i:04}{}", "x".repeat(4092)))
        .collect();
    assert!(FixtureCatalog::new(vec![fixture]).is_err());
    assert!(FixtureManifest::parse(&" ".repeat(1_048_577)).is_err());
    // No source execution, output or marker counts are claimed by reading a contract.
    assert!(
        catalog
            .get("T034-CONTRACT-ON-DISK")
            .unwrap()
            .check(FixtureMode::Off, &FixtureObservation::default())
            .is_err()
    );
}
