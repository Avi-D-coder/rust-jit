use lrr::{
    core::digest::{Digest, StableEncode},
    project::cargo_units::{Selection, UnitContext, UnitGraph, UnitRole},
};
use std::collections::BTreeMap;
#[test]
fn selected_cargo_roots_and_dependencies_preserve_all_official_editions() {
    let mut raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../compat/fixtures/edition2024/h001-unit-graph.json"
    ))
    .unwrap();
    let root = usize::try_from(raw["roots"][0].as_u64().unwrap()).unwrap();
    let target = raw["units"][root]["platform"].as_str().unwrap().to_owned();
    let context = UnitContext {
        package: raw["units"][root]["pkg_id"].as_str().unwrap().into(),
        selection: Selection::Library {
            test: true,
            name: raw["units"][root]["target"]["name"]
                .as_str()
                .unwrap()
                .into(),
        },
        ordinary_role: UnitRole::Live,
        host: target.clone(),
        target: target.clone(),
        cfg_by_target: BTreeMap::from([(target, Digest([204; 32]))]),
        rustc_fingerprint: Digest([205; 32]),
    };
    let baseline = UnitGraph::parse(&serde_json::to_vec(&raw).unwrap(), &context).unwrap();
    for (i, unit) in raw["units"].as_array_mut().unwrap().iter_mut().enumerate() {
        if i != root {
            unit["target"]["edition"] = "2021".into();
        }
    }
    let retained = UnitGraph::parse(&serde_json::to_vec(&raw).unwrap(), &context).unwrap();
    assert!(retained.units.values().any(|u| u.key.edition == 2021));
    for (id, unit) in &retained.units {
        assert_eq!(
            unit.key.edition,
            if retained.roots.contains(id) {
                2024
            } else {
                2021
            }
        );
        assert_eq!(unit.key.fingerprint(), *id);
    }
    assert_ne!(
        baseline.units.keys().collect::<Vec<_>>(),
        retained.units.keys().collect::<Vec<_>>()
    );
    let mut roots = std::collections::BTreeSet::new();
    for edition in [2015, 2018, 2021, 2024] {
        raw["units"][root]["target"]["edition"] = edition.to_string().into();
        let graph = UnitGraph::parse(&serde_json::to_vec(&raw).unwrap(), &context).unwrap();
        let key = *graph.roots.iter().next().unwrap();
        assert_eq!(graph.units[&key].key.edition, edition);
        assert!(roots.insert(key));
        for (id, unit) in &graph.units {
            assert_eq!(unit.key.fingerprint(), *id);
            if *id != key {
                assert_eq!(unit.key.edition, 2021);
            }
        }
    }
    raw["units"][root]["target"]["edition"] = "2099".into();
    assert_eq!(
        UnitGraph::parse(&serde_json::to_vec(&raw).unwrap(), &context)
            .unwrap_err()
            .code,
        "LRR-E-LIVE-EDITION"
    );
}

#[test]
fn incompatible_profile_refresh_and_selection_cannot_reuse_or_mutate_warm_live_publication() {
    use lrr::{
        core::profile::ProfileIdentity,
        db::{
            key::{OutlineKey, QueryKey},
            memo::Database,
            poll::QueryPoll,
        },
        engine::run::{RunMachine, RunPoll},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let blob = SourceBlob::new(b"fn case(){let flag=true;if flag{}}".to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([206; 32]);
    let make_unit = |profile: ProfileIdentity| SourceUnitInput {
        path: path.clone(),
        crate_key: unit,
        module_path: vec![],
        profile,
        cfg: cfg.clone(),
        blob: blob.clone(),
    };
    let prepared =
        PreparedSourceInputs::refresh(None, vec![make_unit(profile.clone())], Default::default())
            .unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("live source absent")
    };
    let body = view.outline().items[0].body.as_ref().unwrap().key();
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
    db.finish_run().unwrap();
    let revision = db.source_revision();
    let events = db.trace.events.len();
    let key = QueryKey::BodyStructure(body.clone());
    let memo = db.memo(&key).unwrap().value.clone();
    for (edition, semantics, code) in [
        (2099, "lrr-2024-v2", "LRR-E-TOOLCHAIN-CAPABILITY"),
        (2024, "lrr-2024-v1", "LRR-E-SEMANTICS-PROFILE"),
    ] {
        let changed = ProfileIdentity {
            edition,
            semantics: semantics.into(),
        };
        let error = match PreparedSourceInputs::refresh(
            Some(&prepared),
            vec![make_unit(changed.clone())],
            Default::default(),
        ) {
            Ok(_) => panic!("incompatible profile admitted"),
            Err(error) => error,
        };
        assert_eq!(
            error.code,
            if edition == 2099 {
                "LRR-E-LIVE-EDITION"
            } else {
                code
            }
        );
        let mut changed_selection = selection.clone();
        changed_selection.outline.parse.profile = changed;
        let error = match prepared.select(&changed_selection) {
            Ok(_) => panic!("incompatible selection admitted"),
            Err(error) => error,
        };
        assert_eq!(error.code, code);
        assert_eq!(db.source_revision(), revision);
        assert_eq!(db.trace.events.len(), events);
        assert!(std::sync::Arc::ptr_eq(&db.memo(&key).unwrap().value, &memo));
    }
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut second)) =
        RunMachine::start_with_sources(&mut db, body, 1000, vec![view.share()])
    else {
        panic!("valid warm run absent")
    };
    assert!(matches!(second.poll(&mut db), RunPoll::Completed(_)));
    db.finish_run().unwrap();
    assert!(std::sync::Arc::ptr_eq(&db.memo(&key).unwrap().value, &memo));
}
