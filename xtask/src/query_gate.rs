//! Gate 01 measures the source/query layer; executable recipes arrive in T082.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{BucketKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::outline::ScopeKey,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const BASE: &str = "fn selected() -> i64 { 1 } fn cold() -> i64 { 2 } fn unrelated() -> i64 { 3 }";
fn source(text: &str) -> Result<SourceUnitInput, String> {
    Ok(SourceUnitInput {
        path: SourcePath::new("lib.rs").map_err(|e| e.to_string())?,
        crate_key: Digest::of_bytes(b"gate01"),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).map_err(|e| e.to_string())?,
    })
}
fn bucket(name: &str) -> QueryKey {
    QueryKey::ScopeBucket(BucketKey {
        scope: ScopeKey {
            crate_key: Digest::of_bytes(b"gate01"),
            module_path: vec![],
            owner: None,
        },
        namespace: Namespace::Value,
        name: name.into(),
    })
}
struct Observed {
    bodies: BTreeMap<String, Digest>,
    headers: BTreeMap<String, Digest>,
    buckets: BTreeMap<String, Digest>,
    missing_count: usize,
}
fn observe(db: &mut Database) -> Result<Observed, String> {
    let mut observed = Observed {
        bodies: BTreeMap::new(),
        headers: BTreeMap::new(),
        buckets: BTreeMap::new(),
        missing_count: 0,
    };
    for name in ["selected", "cold", "unrelated", "missing"] {
        let result = db
            .fetch(&SyntaxProvider, bucket(name))
            .into_ready()
            .expect("fixture query must complete")
            .unwrap();
        let QueryValue::ScopeBucket(value) = result.value.as_ref() else {
            return Err(format!("unexpected name outcome: {:?}", result.value));
        };
        if !value.blockers.is_empty() {
            return Err("edit matrix has unresolved macro blockers".into());
        }
        observed.buckets.insert(name.into(), result.fingerprint);
        if name == "missing" {
            observed.missing_count = value.candidates.len();
            continue;
        }
        if value.candidates.len() != 1 {
            return Err(format!("{name} must have one declaration"));
        }
        let definition = value.candidates[0];
        let body = db
            .input(&InputKey::DefinitionBody(definition))
            .ok_or("body input absent")?;
        let InputValue::DefinitionBody(body) = body.as_ref() else {
            return Err("body input variant mismatch".into());
        };
        observed
            .bodies
            .insert(name.into(), body.key().fingerprint());
        // The cold body's structure is observed, but its signature is not demanded.
        if name != "cold" {
            let signature = db
                .fetch(&SyntaxProvider, QueryKey::Signature(definition))
                .into_ready()
                .expect("fixture query must complete")
                .unwrap();
            if !matches!(signature.value.as_ref(), QueryValue::Signature(_)) {
                return Err("signature query failed".into());
            }
            observed.headers.insert(name.into(), signature.fingerprint);
        }
    }
    Ok(observed)
}
fn count(counts: &BTreeMap<String, u64>, label: &str) -> u64 {
    counts.get(label).copied().unwrap_or(0)
}
fn delta(
    before: &BTreeMap<String, u64>,
    after: &BTreeMap<String, u64>,
) -> Result<BTreeMap<String, u64>, String> {
    after
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .checked_sub(count(before, key))
                    .ok_or("counter regressed")?,
            ))
        })
        .collect()
}
pub fn edit_matrix() -> Result<Value, String> {
    let edits = [
        (
            "called_function_body",
            BASE.replace("{ 1 }", "{ 10 }"),
            "selected",
            false,
            false,
        ),
        (
            "uncalled_function_body",
            BASE.replace("{ 2 }", "{ absent_in_cold_body() }"),
            "cold",
            false,
            false,
        ),
        (
            "called_signature",
            BASE.replace("selected() -> i64", "selected() -> u64"),
            "",
            true,
            false,
        ),
        (
            "previously_missing_name",
            format!("{BASE} fn missing() {{}}"),
            "",
            false,
            true,
        ),
        (
            "source_line_insertion",
            format!("\n\n{BASE}"),
            "",
            false,
            false,
        ),
    ];
    let mut rows = vec![];
    for (name, text, changed_body, changed_header, new_name) in edits {
        let mut db = Database::default();
        db.trace.detailed = true;
        let before = PreparedSourceInputs::refresh(None, vec![source(BASE)?], BTreeMap::new())
            .map_err(|e| e.to_string())?;
        before.publish(&mut db).map_err(|e| e.to_string())?;
        let a = observe(&mut db)?;
        let initial_counts = db.trace.counts.clone();
        if count(&initial_counts, "ScopeBucket:ProviderRun") != 4
            || count(&initial_counts, "Signature:ProviderRun") != 2
        {
            return Err(format!(
                "{name}: initial provider counts differ: {initial_counts:?}"
            ));
        }
        let event_start = db.trace.events.len();
        let after =
            PreparedSourceInputs::refresh(Some(&before), vec![source(&text)?], BTreeMap::new())
                .map_err(|e| e.to_string())?;
        after.publish(&mut db).map_err(|e| e.to_string())?;
        let b = observe(&mut db)?;
        let counts = delta(&initial_counts, &db.trace.counts)?;
        for body in ["selected", "cold", "unrelated"] {
            if (a.bodies[body] != b.bodies[body]) != (body == changed_body) {
                return Err(format!(
                    "{name}: unexpected body identity change for {body}"
                ));
            }
        }
        for header in ["selected", "unrelated"] {
            if (a.headers[header] != b.headers[header]) != (header == "selected" && changed_header)
            {
                return Err(format!("{name}: unexpected header change for {header}"));
            }
        }
        for bucket in ["selected", "cold", "unrelated", "missing"] {
            if (a.buckets[bucket] != b.buckets[bucket]) != (bucket == "missing" && new_name) {
                return Err(format!("{name}: unexpected bucket change for {bucket}"));
            }
        }
        if a.missing_count != 0
            || b.missing_count != usize::from(new_name)
            || count(&counts, "ScopeBucket:ProviderRun") != u64::from(new_name)
            || count(&counts, "ScopeBucket:Invalidated") != u64::from(new_name)
            || count(&counts, "ScopeBucket:ValidatedReuse") != 4 - u64::from(new_name)
            || count(&counts, "Signature:ProviderRun") != u64::from(changed_header)
            || count(&counts, "Signature:Invalidated") != u64::from(changed_header)
            || count(&counts, "Signature:ValidatedReuse") != 2 - u64::from(changed_header)
        {
            return Err(format!(
                "{name}: wrong provider/invalidation/reuse delta: {counts:?}"
            ));
        }
        rows.push(json!({"edit":name, "scope":"source/query identities only; no executable thunk, region, native artifact or positional intrinsic acceptance claimed", "before":{"parsed_files":before.parsed_files,"outlined_files":before.outlined_files,"provider_counts":initial_counts,"body_keys":a.bodies,"headers":a.headers,"buckets":a.buckets},"after":{"parsed_files":after.parsed_files,"outlined_files":after.outlined_files,"reused_files":after.reused_files,"provider_count_delta":counts,"body_keys":b.bodies,"headers":b.headers,"buckets":b.buckets},"events":&db.trace.events[event_start..]}));
    }
    Ok(
        json!({"scope":"Gate 01 source/query component matrix. Full executable edit matrix remains T082.", "rows":rows}),
    )
}
/// An actual retained inner query handle must remain charged after replacement.
pub fn retired_payload_accounting() -> Result<Value, String> {
    let mut db = Database::default();
    let prepared = PreparedSourceInputs::refresh(None, vec![source(BASE)?], BTreeMap::new())
        .map_err(|e| e.to_string())?;
    prepared.publish(&mut db).map_err(|e| e.to_string())?;
    let lookup = db
        .fetch(&SyntaxProvider, bucket("selected"))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ScopeBucket(found) = lookup.value.as_ref() else {
        return Err("missing selected bucket".into());
    };
    let definition = *found
        .candidates
        .first()
        .ok_or("missing selected declaration")?;
    let signature = db
        .fetch(&SyntaxProvider, QueryKey::Signature(definition))
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::Signature(header) = signature.value.as_ref() else {
        return Err("signature failed".into());
    };
    let retained = header.clone();
    drop(signature);
    drop(lookup);
    db.publish_inputs(BTreeMap::new())
        .map_err(|e| e.to_string())?;
    let deleted = db
        .fetch(&SyntaxProvider, QueryKey::Signature(definition))
        .into_ready()
        .expect("deleted signature must be a completed error")
        .unwrap_err();
    if deleted.code != "LRR-E-NAME-MISSING" {
        return Err("wrong deleted-signature diagnostic".into());
    }
    drop(prepared);
    db.evict_between_runs(lrr::engine::telemetry::CacheBudgets::default())
        .map_err(|e| e.to_string())?;
    let before = db.cache_usage().map_err(|e| e.to_string())?;
    let retained_bytes = retained.estimated_bytes();
    drop(retained);
    db.evict_between_runs(lrr::engine::telemetry::CacheBudgets::default())
        .map_err(|e| e.to_string())?;
    let after = db.cache_usage().map_err(|e| e.to_string())?;
    if before
        .memo_bytes
        .checked_sub(after.memo_bytes)
        .is_none_or(|released| released < retained_bytes)
    {
        return Err(format!(
            "retired signature storage escaped accounting: retained estimate={retained_bytes}, before={before:?}, after={after:?}"
        ));
    }
    Ok(
        json!({"retained_header_estimate":retained_bytes,"before_release":before,"after_release":after}),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn retired_signature_payload_remains_charged_until_release() {
        let report = super::retired_payload_accounting().unwrap();
        assert!(
            report["before_release"]["pinned_memo_bytes"]
                .as_u64()
                .unwrap()
                > report["after_release"]["pinned_memo_bytes"]
                    .as_u64()
                    .unwrap()
        );
    }
    #[test]
    fn initial_edit_matrix_reports_real_provider_and_reuse_counts() {
        let report = super::edit_matrix().unwrap();
        assert_eq!(report["rows"].as_array().unwrap().len(), 5);
        for row in report["rows"].as_array().unwrap() {
            assert!(!row["events"].as_array().unwrap().is_empty());
            assert_eq!(row["after"]["parsed_files"], 1);
        }
    }
}
