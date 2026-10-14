//! Local diagnostic explanations; no source capture, query or network effects.
use serde::Deserialize;
use std::collections::BTreeSet;

const MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../compat/diagnostics.toml"
));

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    diagnostic: Vec<Explanation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Explanation {
    code: String,
    phase: String,
    invariant: String,
    effects_may_have_occurred: bool,
    cleanup_runs: bool,
    example: String,
    corrective_command: String,
    fixtures: Vec<String>,
}

fn explain(code: &str) -> Result<Explanation, String> {
    if MANIFEST.len() > 1024 * 1024 {
        return Err("local diagnostic manifest exceeds its byte limit".into());
    }
    let manifest: Manifest = toml::from_str(MANIFEST)
        .map_err(|_| "local diagnostic manifest is malformed".to_owned())?;
    if manifest.schema != 1 || manifest.diagnostic.len() > 4096 {
        return Err("local diagnostic manifest schema or count is unsupported".into());
    }
    let mut identities = BTreeSet::new();
    for entry in &manifest.diagnostic {
        if !entry.code.starts_with("LRR-E-")
            || !identities.insert(&entry.code)
            || entry.fixtures.is_empty()
            || entry.fixtures.len() > 256
            || [
                &entry.code,
                &entry.phase,
                &entry.invariant,
                &entry.example,
                &entry.corrective_command,
            ]
            .into_iter()
            .chain(entry.fixtures.iter())
            .any(|text| text.is_empty() || text.len() > 64 * 1024)
        {
            return Err("local diagnostic manifest contains invalid or duplicate entries".into());
        }
    }
    manifest
        .diagnostic
        .into_iter()
        .find(|entry| entry.code == code)
        .ok_or_else(|| "unknown diagnostic code; use the code printed by lrr".into())
}

pub(super) fn entry(code: &str) -> i32 {
    match explain(code) {
        Ok(entry) => {
            println!(
                "{}\nInvariant: {}\nPhase: {}\nEffects may already have occurred: {}\nCleanup runs: {}\nExample: {}\nNext step: {}",
                entry.code,
                entry.invariant,
                entry.phase,
                entry.effects_may_have_occurred,
                entry.cleanup_runs,
                entry.example,
                entry.corrective_command
            );
            0
        }
        Err(message) => {
            eprintln!("lrr explain: {message}");
            2
        }
    }
}
