//! Service refuses stale or unauthorized contexts before starting any helper.
use lrr::{
    core::digest::{Digest, StableEncode},
    service::{expansion::DemandContext, macro_session::MacroSession},
};
use std::{path::PathBuf, time::Duration};
mod fixture {
    include!("support/tool_manifest_fixture.rs");
}
fn context(fixture: &fixture::Fixture) -> DemandContext {
    let unit = *fixture.manifest.graph.roots.first().unwrap();
    DemandContext {
        request: Digest::of_bytes(b"request"),
        session: Digest::of_bytes(b"session"),
        unit,
        revision: 1,
        epoch: 1,
        source_generation: Digest::of_bytes(b"snapshot"),
        native_generation: None,
        compile_environment: fixture.manifest.compile_env[&unit].fingerprint(),
    }
}
#[test]
fn service_macro_authority_precedes_preparation_validation_or_io() {
    let mut fixture = fixture::Fixture::new();
    let context = context(&fixture);
    fixture.manifest.schema_version = 0;
    fixture.manifest.toolchain.macro_server.path = "missing-relative".into();
    let error = MacroSession::open(
        context,
        &fixture.manifest,
        false,
        PathBuf::from("missing"),
        Duration::ZERO,
        0,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-MACRO-PERMISSION");
}
#[test]
fn service_macro_stale_environment_and_absent_consumer_refuse_before_helper() {
    let fixture = fixture::Fixture::new();
    let context = context(&fixture);
    let mut changed = context.clone();
    changed.compile_environment = Digest::of_bytes(b"different");
    let error = MacroSession::open(
        changed,
        &fixture.manifest,
        true,
        fixture.root.clone(),
        Duration::from_secs(1),
        4096,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-TOOL-PREPARE-STALE");
    let mut changed = context;
    changed.unit = Digest::of_bytes(b"absent");
    let error = MacroSession::open(
        changed,
        &fixture.manifest,
        true,
        fixture.root.clone(),
        Duration::from_secs(1),
        4096,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-UNIT-GRAPH");
}
