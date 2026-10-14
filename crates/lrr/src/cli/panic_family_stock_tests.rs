//! Actual unchanged main execution components; configured run-fail acceptance is separate.
use super::{StandaloneOutcome, standalone_edition_with_capture};
use crate::core::digest::Digest;
fn original(path: &str, expected: &str) -> std::path::PathBuf {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout");
    let file = root.join(path);
    assert_eq!(
        std::fs::read(&file).unwrap(),
        expected.as_bytes(),
        "original fixture changed"
    );
    file
}
#[test]
fn unchanged_rustc_panic_family_mains_have_actual_guest_messages_all_editions() {
    for (name, source) in [
        (
            "unreachable-macro-panic.rs",
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/unreachable-macro-panic.rs"
            ),
        ),
        (
            "unreachable-static-msg.rs",
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/unreachable-static-msg.rs"
            ),
        ),
        (
            "unimplemented-macro-panic.rs",
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/unimplemented-macro-panic.rs"
            ),
        ),
    ] {
        let path = original(&format!("tests/ui/macros/{name}"), source);
        let expected_message = source
            .lines()
            .find_map(|line| line.strip_prefix("//@ error-pattern:"))
            .expect("unchanged upstream runtime message assertion");
        for edition in [2015, 2018, 2021, 2024] {
            let StandaloneOutcome::Captured(packet) =
                standalone_edition_with_capture(&path, None, edition, true, None).unwrap()
            else {
                panic!("actual requested capture required")
            };
            assert!(packet.guest_complete(), "{name}/{edition}");
            assert_eq!(packet.exit_code(), 1);
            assert_eq!(packet.guest_stdout(), Some(&[][..]));
            assert_eq!(packet.guest_stderr(), Some(&[][..]));
            let value = serde_json::to_value(&packet).unwrap();
            assert_eq!(value["terminal"]["status"], "guest_panic");
            assert_eq!(
                value["capture"]["facts"]["terminal"]["message"],
                expected_message
            );
            assert_eq!(value["snapshot"]["owning_edition"], edition);
            assert!(value["capture"]["facts"]["vm_dispatches"].as_u64().unwrap() > 0);
            assert!(
                value["snapshot"]["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["content"]
                        == serde_json::to_value(Digest::of_bytes(source.as_bytes())).unwrap())
            );
            // Configured subprocess, standard banner and original exit expectations
            // are not supplied by this component capture; no full fixture pass.
        }
    }
}
