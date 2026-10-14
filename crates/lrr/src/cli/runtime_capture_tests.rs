//! Preregistered transport/runtime components, not compiletest configurations.
use super::{StandaloneOutcome, runtime_capture, standalone_edition_with_capture};
use crate::core::digest::Digest;
use std::path::{Path, PathBuf};
fn fixtures() -> PathBuf {
    std::env::var_os("LRR_CLI_CAPTURE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout")
        })
}
fn packet(path: &Path) -> runtime_capture::Packet {
    let outcome = standalone_edition_with_capture(path, None, 2015, true, None)
        .expect("capture route must produce a typed packet");
    let StandaloneOutcome::Captured(packet) = outcome else {
        panic!("capture requested but legacy result returned")
    };
    *packet
}
// Exact preserved originals, not expected guest outputs or source adapters.
// MIT OR Apache-2.0, rustc ae0fe511b15ab760d7e963b13389d1bc2ab42b18;
// both licenses and complete unchanged files retained in the fixture packet.
const STOCK_BYTES: &[(&str, &[u8])] = &[
    ("tests/ui/numbers-arithmetic/floatlits.rs", b"\x2f\x2f\x40\x20\x72\x75\x6e\x2d\x70\x61\x73\x73\x0a\x0a\x0a\x0a\x70\x75\x62\x20\x66\x6e\x20\x6d\x61\x69\x6e\x28\x29\x20\x7b\x0a\x20\x20\x20\x20\x6c\x65\x74\x20\x66\x20\x3d\x20\x34\x2e\x39\x39\x39\x39\x39\x39\x39\x39\x39\x39\x39\x39\x66\x36\x34\x3b\x0a\x20\x20\x20\x20\x61\x73\x73\x65\x72\x74\x21\x28\x66\x20\x3e\x20\x34\x2e\x39\x30\x66\x36\x34\x29\x3b\x0a\x20\x20\x20\x20\x61\x73\x73\x65\x72\x74\x21\x28\x66\x20\x3c\x20\x35\x2e\x30\x66\x36\x34\x29\x3b\x0a\x20\x20\x20\x20\x6c\x65\x74\x20\x67\x20\x3d\x20\x34\x2e\x39\x30\x30\x30\x30\x30\x30\x30\x30\x30\x31\x65\x2d\x31\x30\x66\x36\x34\x3b\x0a\x20\x20\x20\x20\x61\x73\x73\x65\x72\x74\x21\x28\x67\x20\x3e\x20\x35\x65\x2d\x31\x31\x66\x36\x34\x29\x3b\x0a\x20\x20\x20\x20\x61\x73\x73\x65\x72\x74\x21\x28\x67\x20\x3c\x20\x35\x65\x2d\x39\x66\x36\x34\x29\x3b\x0a\x7d\x0a"),
    ("tests/ui/numbers-arithmetic/i8-incr.rs", b"\x2f\x2f\x40\x20\x72\x75\x6e\x2d\x70\x61\x73\x73\x0a\x0a\x0a\x0a\x0a\x70\x75\x62\x20\x66\x6e\x20\x6d\x61\x69\x6e\x28\x29\x20\x7b\x0a\x20\x20\x20\x20\x6c\x65\x74\x20\x6d\x75\x74\x20\x78\x3a\x20\x69\x38\x20\x3d\x20\x2d\x31\x32\x3b\x0a\x20\x20\x20\x20\x6c\x65\x74\x20\x79\x3a\x20\x69\x38\x20\x3d\x20\x2d\x31\x32\x3b\x0a\x20\x20\x20\x20\x78\x20\x3d\x20\x78\x20\x2b\x20\x31\x3b\x0a\x20\x20\x20\x20\x78\x20\x3d\x20\x78\x20\x2d\x20\x31\x3b\x0a\x20\x20\x20\x20\x61\x73\x73\x65\x72\x74\x5f\x65\x71\x21\x28\x78\x2c\x20\x79\x29\x3b\x0a\x7d\x0a"),
    ("tests/ui/cast/u8-to-char-cast-9918.rs", b"\x2f\x2f\x20\x68\x74\x74\x70\x73\x3a\x2f\x2f\x67\x69\x74\x68\x75\x62\x2e\x63\x6f\x6d\x2f\x72\x75\x73\x74\x2d\x6c\x61\x6e\x67\x2f\x72\x75\x73\x74\x2f\x69\x73\x73\x75\x65\x73\x2f\x39\x39\x31\x38\x0a\x2f\x2f\x40\x20\x72\x75\x6e\x2d\x70\x61\x73\x73\x0a\x0a\x70\x75\x62\x20\x66\x6e\x20\x6d\x61\x69\x6e\x28\x29\x20\x7b\x0a\x20\x20\x20\x20\x61\x73\x73\x65\x72\x74\x5f\x65\x71\x21\x28\x28\x30\x20\x2b\x20\x30\x75\x38\x29\x20\x61\x73\x20\x63\x68\x61\x72\x2c\x20\x27\x5c\x30\x27\x29\x3b\x0a\x7d\x0a"),
    ("tests/ui/panics/panic-macro-static.rs", b"\x2f\x2f\x40\x20\x72\x75\x6e\x2d\x66\x61\x69\x6c\x0a\x2f\x2f\x40\x20\x65\x72\x72\x6f\x72\x2d\x70\x61\x74\x74\x65\x72\x6e\x3a\x70\x61\x6e\x69\x63\x6b\x65\x64\x0a\x2f\x2f\x40\x20\x65\x72\x72\x6f\x72\x2d\x70\x61\x74\x74\x65\x72\x6e\x3a\x74\x65\x73\x74\x2d\x66\x61\x69\x6c\x2d\x73\x74\x61\x74\x69\x63\x0a\x2f\x2f\x40\x20\x6e\x65\x65\x64\x73\x2d\x73\x75\x62\x70\x72\x6f\x63\x65\x73\x73\x0a\x0a\x66\x6e\x20\x6d\x61\x69\x6e\x28\x29\x20\x7b\x0a\x20\x20\x20\x20\x70\x61\x6e\x69\x63\x21\x28\x22\x74\x65\x73\x74\x2d\x66\x61\x69\x6c\x2d\x73\x74\x61\x74\x69\x63\x22\x29\x3b\x0a\x7d\x0a"),
];
fn stock_packet(case: &str) -> runtime_capture::Packet {
    let expected = STOCK_BYTES
        .iter()
        .find(|(path, _)| *path == case)
        .unwrap()
        .1;
    let path = fixtures().join(case);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        expected,
        "upstream fixture bytes changed"
    );
    let packet = packet(&path);
    let json = serde_json::to_value(&packet).unwrap();
    let content = serde_json::to_value(Digest::of_bytes(expected)).unwrap();
    assert!(
        json["snapshot"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["content"] == content),
        "executed snapshot did not retain the unchanged original"
    );
    packet
}
#[test]
fn unchanged_supported_numeric_entries_complete_with_closed_capture() {
    // Chosen standalone edition is explicit LRR transport/component scope;
    // absent actual compiletest matrix means no official configuration pass.
    for case in [
        "tests/ui/numbers-arithmetic/floatlits.rs",
        "tests/ui/numbers-arithmetic/i8-incr.rs",
        "tests/ui/cast/u8-to-char-cast-9918.rs",
    ] {
        let packet = stock_packet(case);
        assert_eq!(packet.exit_code(), 0, "{case}");
        assert!(packet.guest_complete(), "{case}");
        assert_eq!(packet.guest_stdout(), Some(&[][..]));
        assert_eq!(packet.guest_stderr(), Some(&[][..]));
        let json = serde_json::to_value(&packet).unwrap();
        assert_eq!(
            json["transport_stdout_role"],
            "protocol_json_not_guest_stdout"
        );
        assert_eq!(json["terminal"]["status"], "completed");
        assert_eq!(json["terminal"]["value"]["kind"], "unit");
        assert_eq!(json["snapshot"]["owning_edition"], 2015);
        assert!(json["capture"]["facts"]["vm_dispatches"].as_u64().unwrap() > 0);
    }
}
#[test]
fn unchanged_static_panic_is_guest_terminal_not_cli_transport_stderr() {
    let packet = stock_packet("tests/ui/panics/panic-macro-static.rs");
    assert_eq!(packet.exit_code(), 1);
    assert!(packet.guest_complete());
    assert_eq!(packet.guest_stderr(), Some(&[][..]));
    let json = serde_json::to_value(packet).unwrap();
    assert_eq!(json["terminal"]["status"], "guest_panic");
    assert_eq!(
        json["capture"]["facts"]["terminal"]["message"],
        "test-fail-static"
    );
    // No synthesized standard Rust panic banner/exit101. Upstream run-fail
    // status and error-pattern expectations remain separate pending contracts.
}
#[test]
fn capture_context_parser_has_exact_lowercase_digest_boundary() {
    let valid = "ab".repeat(32);
    assert_eq!(
        super::args::parse_capture_context(&valid).unwrap(),
        Digest([0xab; 32])
    );
    for bad in [
        "A".repeat(64),
        "a".repeat(63),
        "g".repeat(64),
        format!("{valid}\n"),
    ] {
        assert!(super::args::parse_capture_context(&bad).is_err());
    }
}
#[test]
fn capture_flags_are_run_only_and_context_requires_capture() {
    use clap::Parser;
    assert!(super::args::Args::try_parse_from(["lrr", "run", "x.rs", "--runtime-capture"]).is_ok());
    assert!(
        super::args::Args::try_parse_from(["lrr", "test", "x.rs", "--runtime-capture"]).is_err()
    );
    assert!(
        super::args::Args::try_parse_from([
            "lrr",
            "run",
            "x.rs",
            "--capture-context",
            &"a".repeat(64)
        ])
        .is_err()
    );
}
#[test]
fn observed_fault_and_missing_producer_never_fabricate_empty_streams() {
    let diagnostic = crate::core::diagnostic::Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "output sink unavailable".into(),
    );
    let packet = runtime_capture::Packet::preparation_fault(diagnostic, None).unwrap();
    assert_eq!(packet.exit_code(), 2);
    assert!(!packet.guest_complete());
    assert_eq!(packet.guest_stdout(), None);
    assert_eq!(packet.guest_stderr(), None);
}
#[test]
fn reached_output_without_sink_is_explicitly_incomplete() {
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lrr-capture-output-refusal-{}-{}.rs",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    struct OwnedSource(std::path::PathBuf);
    impl Drop for OwnedSource {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let owned = OwnedSource(path);
    file.write_all(b"fn main() { println!(); }\n").unwrap();
    drop(file);
    let packet = packet(&owned.0);
    assert_eq!(packet.exit_code(), 2);
    assert!(!packet.guest_complete());
    assert_eq!(packet.guest_stdout(), None);
    assert_eq!(packet.guest_stderr(), None);
    let json = serde_json::to_value(packet).unwrap();
    assert_eq!(json["terminal"]["code"], "LRR-E-UNSUPPORTED");
    assert!(
        json["terminal"]["message"]
            .as_str()
            .unwrap()
            .contains("managed output integration")
    );
}
