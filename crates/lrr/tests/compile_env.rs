use lrr::{
    project::tool_manifest::{CompileEnv, PackageEnvironments},
    source::snapshot::CfgSet,
};
use std::collections::BTreeMap;
fn metadata() -> serde_json::Value {
    serde_json::json!({"version":1,"packages":[{"id":"package@1.2.3-alpha.1+build.9","name":"package","version":"1.2.3-alpha.1+build.9","manifest_path":"/fixture/Cargo.toml","authors":["Avi","Zoë"],"description":null,"homepage":null,"repository":null,"license":null,"license_file":null,"rust_version":null,"readme":null,"features":{"feature":["dep:item","dep:item"]},"metadata":{"float":1.5}}]})
}
#[test]
fn cargo_metadata_is_bounded_duplicate_checked_and_rejects_invalid_versions() {
    let value = metadata();
    PackageEnvironments::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    let text = serde_json::to_string(&value).unwrap();
    let duplicate = text.replacen("\"authors\":", "\"authors\":[],\"authors\":", 1);
    assert!(
        PackageEnvironments::decode(duplicate.as_bytes())
            .unwrap_err()
            .message
            .contains("duplicate")
    );
    let mut repeated = value.clone();
    repeated["packages"]
        .as_array_mut()
        .unwrap()
        .push(value["packages"][0].clone());
    assert!(PackageEnvironments::decode(&serde_json::to_vec(&repeated).unwrap()).is_err());
    for version in [
        "1.2",
        "01.2.3",
        "1.2.3-",
        "1.2.3+",
        "1.2.3-alpha..1",
        "1.2.3-01",
        "1.2.3+bad?",
        "18446744073709551616.2.3",
    ] {
        let mut invalid = value.clone();
        invalid["packages"][0]["version"] = serde_json::json!(version);
        assert!(
            PackageEnvironments::decode(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{version}"
        );
    }
    assert!(PackageEnvironments::decode(&vec![b' '; 16 * 1024 * 1024 + 1]).is_err());
}
#[test]
fn captured_cfg_facts_use_rust_literals_and_invalid_input_is_atomic() {
    let mut cfg = CfgSet::default();
    for input in [
        "unix",
        r#"target_os="macos""#,
        r##"feature=r#"quoted\\value"#"##,
        r#"value="line\n\u{3bb}""#,
    ] {
        lrr::syntax::cfg_input::add(&mut cfg, input).unwrap();
    }
    assert!(cfg.atoms.contains("unix"));
    assert!(cfg.values["target_os"].contains("macos"));
    assert!(cfg.values["value"].contains("line\nλ"));
    let before = cfg.clone();
    for input in [
        "all(unix)",
        "name=7",
        "name=\"\\q\"",
        "unix,",
        "unix)] fn injected() {} //",
        "x; y",
        "",
    ] {
        assert!(
            lrr::syntax::cfg_input::add(&mut cfg, input).is_err(),
            "{input}"
        );
        assert_eq!(cfg, before);
    }
}
#[test]
fn compile_env_precedence_preserves_empty_and_absent_without_ambient_fallback() {
    let mut env = CompileEnv {
        unit: Default::default(),
        host: "aarch64-apple-darwin".into(),
        target: "aarch64-apple-darwin".into(),
        cfg: CfgSet::default(),
        cargo: BTreeMap::from([("KEY".into(), "cargo".into())]),
        rustc_env: BTreeMap::from([("KEY".into(), String::new())]),
        declared: BTreeMap::from([
            ("KEY".into(), Some("declared".into())),
            ("ABSENT".into(), None),
        ]),
        out_dir: None,
    };
    assert_eq!(env.lookup("KEY"), Some(""));
    env.rustc_env.clear();
    assert_eq!(env.lookup("KEY"), Some("cargo"));
    env.cargo.clear();
    assert_eq!(env.lookup("KEY"), Some("declared"));
    assert_eq!(env.lookup("ABSENT"), None);
    assert_eq!(env.lookup("PATH"), None);
}
