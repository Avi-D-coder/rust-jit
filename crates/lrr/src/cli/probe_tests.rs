//! Developer probe: run one guest file through the standalone CLI path with
//! full test-harness backtraces. Ignored by default; select it explicitly:
//! `LRR_PROBE_FILE=path LRR_PROBE_EDITION=2021 <test-bin> cli::probe --ignored --nocapture`.
#[test]
#[ignore]
fn probe_file() {
    let Some(path) = std::env::var_os("LRR_PROBE_FILE") else {
        return;
    };
    let edition = std::env::var("LRR_PROBE_EDITION")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(crate::core::profile::LIVE_EDITION);
    match super::standalone_edition(std::path::Path::new(&path), None, edition) {
        Ok(_) => eprintln!("LRR-PROBE ok"),
        Err(error) => eprintln!("LRR-PROBE error: {error}"),
    }
}
