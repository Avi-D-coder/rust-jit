use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    source::{
        manifest::{CrateKey, PackageKey, TargetKind},
        snapshot::{CfgSet, ProjectManifest, Snapshot, SourceBlob, SourcePath},
    },
    syntax::{
        parse::ParsedFile,
        spans::{BodySpan, SpanAttachments},
    },
};
use std::collections::BTreeMap;

fn snapshot(text: &str) -> Snapshot {
    let cfg = CfgSet::guest_default();
    let key = CrateKey {
        package: PackageKey {
            source_identity: "fixture".into(),
            package_name: "fixture".into(),
            version: "0.1.0".into(),
        },
        target_kind: TargetKind::LibraryTests,
        target_name: "fixture".into(),
        guest_edition: 2024,
        semantics_profile: ProfileIdentity::current().fingerprint(),
        cfg_fingerprint: cfg.fingerprint(),
        dependency_aliases: Digest::of_bytes(b""),
    };
    let selected_crate = key.fingerprint();
    let path = SourcePath::new("main.rs").unwrap();
    Snapshot::new(
        ProjectManifest {
            identity: "fixture".into(),
            root_source: path.clone(),
            selected_crate,
            crates: BTreeMap::from([(selected_crate, key)]),
        },
        BTreeMap::from([(path, SourceBlob::new(text.as_bytes().to_vec()).unwrap())]),
        BTreeMap::from([(selected_crate, cfg)]),
        BTreeMap::new(),
        None,
    )
    .unwrap()
}

#[test]
fn source_lease_pins_parse_and_current_source_map_and_releases_together() {
    use lrr::{
        db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            memo::Database,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        engine::telemetry::CacheBudgets,
        syntax::parse::ParseKey,
    };
    use std::sync::Arc;
    let snapshot = Arc::new(snapshot("fn target() { 1; }"));
    let file = SourcePath::new("main.rs").unwrap();
    let blob = snapshot.files()[&file].clone();
    let bytes = blob.bytes().len();
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::SourceBlob(blob.content()),
        InputValue::SourceBlob(blob.clone()),
    )]))
    .unwrap();
    let key = QueryKey::ParseFile(ParseKey {
        content: blob.content(),
        profile: ProfileIdentity::current(),
    });
    let result = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ParseFile(parsed) = result.value.as_ref() else {
        panic!("expected parse")
    };
    let outline = parsed
        .outline(
            snapshot.project().selected_crate,
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    let mut attachments = SpanAttachments::new(&snapshot);
    attachments
        .insert_body(&body.key(), file.clone(), body.range)
        .unwrap();
    let span = BodySpan {
        body_key: body.key(),
        relative_start: 0,
        relative_end: 1,
    };
    let lease = db
        .pin_source_lease(
            parsed.clone(),
            snapshot.clone(),
            Arc::new(attachments),
            &file,
        )
        .unwrap();
    drop(result);
    let usage = db.cache_usage().unwrap();
    assert_eq!(usage.source_input_bytes, bytes); // shared source storage is charged once
    assert!(usage.pinned_parse_bytes > 0);
    assert_eq!(lease.body(&span).unwrap().start.line, 1);
    let budgets = CacheBudgets {
        parse_bytes: 0,
        ..CacheBudgets::default()
    };
    assert_eq!(
        db.evict_between_runs(budgets).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    // Retiring the memo must not stop charging storage retained by the lease.
    db.publish_inputs(BTreeMap::new()).unwrap();
    let replacement = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap_err();
    assert_eq!(replacement.code, "LRR-E-INTERNAL-SNAPSHOT");
    drop(replacement);
    let retired = db.cache_usage().unwrap();
    assert_eq!(retired.source_input_bytes, bytes);
    assert!(retired.pinned_parse_bytes >= lease.parsed().estimated_bytes());
    assert_eq!(retired.pinned_parse_bytes, usage.pinned_parse_bytes);
    assert_eq!(lease.body(&span).unwrap().start.line, 1);
    assert_eq!(
        db.begin_run_with_budgets(budgets).unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    drop(lease);
    drop(snapshot);
    assert!(db.evict_between_runs(budgets).unwrap().victims.is_empty());
    assert_eq!(db.cache_usage().unwrap().parse_bytes, 0);
    assert_eq!(db.cache_usage().unwrap().source_input_bytes, 0);
    assert_eq!(
        db.evict_between_runs(CacheBudgets {
            query_bytes: 0,
            ..budgets
        })
        .unwrap()
        .victims,
        vec![key]
    );
}

#[test]
fn retained_snapshot_compile_environment_counts_toward_metadata_cap() {
    use lrr::{db::memo::Database, engine::telemetry::CacheBudgets};
    use std::sync::Arc;
    let base = snapshot("fn target() {}");
    let snapshot = Arc::new(
        Snapshot::new(
            base.project().clone(),
            base.files().clone(),
            base.cfg_by_crate().clone(),
            BTreeMap::from([("LARGE".into(), "x".repeat(16 * 1024))]),
            None,
        )
        .unwrap(),
    );
    let file = SourcePath::new("main.rs").unwrap();
    let parsed =
        Arc::new(ParsedFile::parse(&snapshot.files()[&file], ProfileIdentity::current()).unwrap());
    let attachments = Arc::new(SpanAttachments::new(&snapshot));
    let mut db = Database::default();
    let lease = db
        .pin_source_lease(parsed, snapshot, attachments, &file)
        .unwrap();
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= 16 * 1024);
    assert_eq!(
        db.begin_run_with_budgets(CacheBudgets {
            query_bytes: 8 * 1024,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    drop(lease);
    let report = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            source_input_bytes: 0,
        })
        .unwrap();
    assert_eq!(report.after.memo_bytes, 0);
    assert_eq!(report.after.parse_bytes, 0);
}

#[test]
fn released_lease_does_not_leave_cached_parse_registry_pinned() {
    use lrr::{
        db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            memo::Database,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        engine::telemetry::CacheBudgets,
        syntax::parse::ParseKey,
    };
    use std::sync::Arc;
    let snapshot = Arc::new(snapshot("fn target() {}"));
    let file = SourcePath::new("main.rs").unwrap();
    let blob = snapshot.files()[&file].clone();
    let key = QueryKey::ParseFile(ParseKey {
        content: blob.content(),
        profile: ProfileIdentity::current(),
    });
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::SourceBlob(blob.content()),
        InputValue::SourceBlob(blob),
    )]))
    .unwrap();
    let result = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .expect("fixture query must complete")
        .unwrap();
    let QueryValue::ParseFile(parsed) = result.value.as_ref() else {
        panic!("expected parse")
    };
    let attachments = Arc::new(SpanAttachments::new(&snapshot));
    let lease = db
        .pin_source_lease(parsed.clone(), snapshot, attachments, &file)
        .unwrap();
    drop(result);
    drop(lease);
    // Active input metadata is pinned too; remove it before demanding a zero cap.
    db.publish_inputs(BTreeMap::new()).unwrap();
    let report = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap();
    assert_eq!(report.victims, vec![key]);
    assert_eq!(report.after.pinned_memo_bytes, 0);
    assert_eq!(report.after.parse_bytes, 0);
}

#[test]
fn line_insertions_reattach_unchanged_body_origins() {
    let old = snapshot("fn earlier(){1_i64;}\nfn target(){missing();}");
    let new = snapshot("\n\nfn earlier(){\n1_i64;\n}\nfn target(){missing();}");
    let path = SourcePath::new("main.rs").unwrap();
    let outline = |snapshot: &Snapshot| {
        ParsedFile::parse(&snapshot.files()[&path], ProfileIdentity::current())
            .unwrap()
            .outline(
                snapshot.project().selected_crate,
                vec![],
                &CfgSet::guest_default(),
            )
            .unwrap()
    };
    let a = outline(&old);
    let b = outline(&new);
    let a = a
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "target")
        .unwrap();
    let b = b
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "target")
        .unwrap();
    assert_eq!(a.def_key.fingerprint(), b.def_key.fingerprint());
    assert_eq!(a.header.fingerprint(), b.header.fingerprint());
    let a = a.body.as_ref().unwrap();
    let b = b.body.as_ref().unwrap();
    assert_eq!(a.key(), b.key());
    let mut old_map = SpanAttachments::new(&old);
    old_map
        .insert_body(&a.key(), path.clone(), a.range)
        .unwrap();
    let mut new_map = SpanAttachments::new(&new);
    new_map.insert_body(&b.key(), path, b.range).unwrap();
    let span = BodySpan {
        body_key: a.key(),
        relative_start: 1,
        relative_end: 8,
    };
    assert_eq!(old_map.body(&old, &span).unwrap().start.line, 2);
    assert_eq!(new_map.body(&new, &span).unwrap().start.line, 6);
    assert_eq!(
        old_map.body(&new, &span).unwrap_err().code,
        "LRR-E-INTERNAL-SOURCE-MAP"
    );
    assert!(
        new_map
            .body(
                &new,
                &BodySpan {
                    relative_end: u32::MAX,
                    ..span
                }
            )
            .is_err()
    );
}

#[test]
fn recoverable_cold_parse_diagnostics_do_not_block_other_declarations() {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(b"fn cold(){let bad = ;} fn live() {}".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(!parsed.errors.is_empty());
    let outline = parsed
        .outline(
            Digest::of_bytes(b"fixture"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    assert_eq!(outline.items.len(), 2);
    outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "live")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .require_entry()
        .unwrap();
}

#[test]
fn token_spans_retain_snapshot_text_origin_and_reject_wrong_unit_or_revision() {
    use lrr::syntax::spans::TokenSpanLease;
    use lrr::syntax::token_wire::{
        ContextRecord, Edition, Key, SourceIdentity, SpanRecord, Transparency,
    };
    use std::sync::Arc;
    let old = Arc::new(snapshot("fn f() { let café = 1; }"));
    let unit = old.project().selected_crate;
    let path = SourcePath::new("main.rs").unwrap();
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key(unit.0),
        edition: Edition::E2024,
    };
    let start = old.files()[&path].text().unwrap().find("café").unwrap() as u32;
    let span = &SpanRecord {
        source: SourceIdentity {
            unit: Key(unit.0),
            content: Key(old.files()[&path].content().0),
            path: path.as_str().into(),
        },
        start,
        end: start + 5,
        context: context.key(),
        edition: Edition::E2024,
        origin: Key([42; 32]),
    };
    let lease = TokenSpanLease::new(old.clone(), unit);
    assert_eq!(lease.source_text(span).unwrap(), "café");
    let original_location = lease.attach(span).unwrap().start;
    assert_eq!(span.origin, Key([42; 32]));
    let changed = Arc::new(snapshot(
        "

fn f() { let café = 2; }",
    ));
    assert!(TokenSpanLease::new(changed, unit).attach(span).is_err());
    assert!(
        TokenSpanLease::new(old.clone(), Digest([99; 32]))
            .attach(span)
            .is_err()
    );
    let mut bad = span.clone();
    bad.end -= 1;
    assert!(lease.source_text(&bad).is_err());
    drop(old);
    assert_eq!(lease.source_text(span).unwrap(), "café");
    assert_eq!(lease.attach(span).unwrap().start, original_location);
}

#[test]
fn rustc_macro_definition_header_uses_actual_keyword_and_name_tokens_after_edits() {
    let original = include_str!(
        "../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
    );
    for (prefix, expected_line) in [("", 2), ("\n\n", 4)] {
        // The unchanged stock program remains the exact suffix. Only current
        // physical layout changes in the explicitly labelled LRR variant.
        let source = format!("{prefix}{original}");
        let captured = snapshot(&source);
        let project = captured.project();
        let blob = &captured.files()[&project.root_source];
        let parsed = ParsedFile::parse(blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(project.selected_crate, vec![], &CfgSet::guest_default())
            .unwrap();
        let definitions: Vec<_> = outline
            .items
            .iter()
            .filter(|item| item.def_key.item_kind == lrr::source::manifest::ItemKind::Macro)
            .collect();
        assert_eq!(definitions.len(), 1);
        let definition = definitions[0];
        let (header, name) = parsed
            .macro_definition_header_at(definition.declaration_span)
            .unwrap();
        assert_eq!(name, definition.def_key.declared_name);
        assert_eq!(name, "one_arg_macro");
        assert_eq!(
            blob.text()
                .unwrap()
                .get(header.start as usize..header.end as usize),
            Some("macro_rules! one_arg_macro")
        );
        let point = captured
            .source_map()
            .locate(captured.files(), &project.root_source, header.start)
            .unwrap();
        assert_eq!((point.line, point.column), (expected_line, 1));
        assert!(
            header.start > definition.header_span.start,
            "leading comment used as definition location"
        );
        let mut partial = definition.declaration_span;
        partial.start += 1;
        assert_eq!(
            parsed.macro_definition_header_at(partial).unwrap_err().code,
            "LRR-E-MACRO-SCOPE"
        );
    }
}
