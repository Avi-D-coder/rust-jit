use lrr::{
    core::digest::{Digest, StableEncode},
    db::seals::SealDomain,
    expand::discovery::{
        Discovery, DiscoveryJob, DiscoveryKind, DiscoveryLimits, DiscoveryStep, DiscoveryUpdate,
    },
};
fn id(n: u8) -> Digest {
    Digest([n; 32])
}
fn job(n: u8, offset: u32) -> DiscoveryJob {
    DiscoveryJob {
        identity: id(n),
        module_order: 0,
        source_offset: offset,
        kind: DiscoveryKind::ResolveMacro,
    }
}
fn work(limits: DiscoveryLimits) -> Discovery {
    Discovery::new(SealDomain::TestCatalog { id: id(1) }, id(2), 1, limits).unwrap()
}
fn next(w: &mut Discovery) -> lrr::expand::discovery::JobTicket {
    match w.poll().unwrap() {
        DiscoveryStep::Run(t) => t,
        _ => panic!("job absent"),
    }
}
#[test]
fn discovery_jobs_follow_source_order_and_completed_jobs_are_not_replayed() {
    let mut w = work(Default::default());
    w.enqueue(job(3, 20)).unwrap();
    w.enqueue(job(4, 10)).unwrap();
    let t = next(&mut w);
    assert_eq!(t.job().identity, id(4));
    assert_eq!(next(&mut w), t);
    w.complete(&t, DiscoveryUpdate::default()).unwrap();
    assert!(w.complete(&t, DiscoveryUpdate::default()).is_err());
    let t = next(&mut w);
    assert_eq!(t.job().identity, id(3));
    w.complete(&t, DiscoveryUpdate::default()).unwrap();
    assert!(matches!(w.poll().unwrap(), DiscoveryStep::Sealed(_)));
}
#[test]
fn discovery_blocked_producers_prevent_seals_and_membership_changes_wake_them() {
    let mut w = work(Default::default());
    w.enqueue(job(3, 0)).unwrap();
    let t = next(&mut w);
    w.block(&t, vec![id(8)]).unwrap();
    let DiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = w.poll().unwrap()
    else {
        panic!("partial seal published")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(blocked, vec![(id(3), vec![id(8)])]);
    w.enqueue(job(4, 10)).unwrap();
    let t = next(&mut w);
    w.complete(
        &t,
        DiscoveryUpdate {
            members: vec![id(8)],
            ..Default::default()
        },
    )
    .unwrap();
    let t = next(&mut w);
    assert_eq!(t.job().identity, id(3));
    w.complete(&t, DiscoveryUpdate::default()).unwrap();
    assert!(matches!(w.poll().unwrap(), DiscoveryStep::Sealed(_)));
}
#[test]
fn discovery_seals_hash_all_members_and_inputs_and_reject_late_membership() {
    let mut a = work(Default::default());
    a.enqueue(job(3, 0)).unwrap();
    let t = next(&mut a);
    a.complete(
        &t,
        DiscoveryUpdate {
            members: vec![id(8), id(7)],
            dependencies: vec![(id(10), id(11))],
            ..Default::default()
        },
    )
    .unwrap();
    let DiscoveryStep::Sealed(s) = a.poll().unwrap() else {
        panic!("seal absent")
    };
    assert_eq!(s.unresolved_frontiers(), 0);
    assert!(a.enqueue(job(5, 20)).is_err());
    let mut b = work(Default::default());
    b.enqueue(job(3, 0)).unwrap();
    let t = next(&mut b);
    b.complete(
        &t,
        DiscoveryUpdate {
            members: vec![id(7), id(8)],
            dependencies: vec![(id(10), id(11))],
            ..Default::default()
        },
    )
    .unwrap();
    let DiscoveryStep::Sealed(other) = b.poll().unwrap() else {
        panic!("seal absent")
    };
    assert_eq!(s.fingerprint(), other.fingerprint());
    let mut c = work(Default::default());
    c.enqueue(job(3, 0)).unwrap();
    let t = next(&mut c);
    c.complete(
        &t,
        DiscoveryUpdate {
            members: vec![id(7), id(8)],
            dependencies: vec![(id(10), id(12))],
            ..Default::default()
        },
    )
    .unwrap();
    let DiscoveryStep::Sealed(other) = c.poll().unwrap() else {
        panic!("seal absent")
    };
    assert_ne!(s.fingerprint(), other.fingerprint());
}
#[test]
fn discovery_limit_and_conflicting_dependency_refusal_preserve_active_work() {
    let mut w = work(DiscoveryLimits {
        jobs: 1,
        ..Default::default()
    });
    w.enqueue(job(3, 0)).unwrap();
    let t = next(&mut w);
    assert_eq!(
        w.complete(
            &t,
            DiscoveryUpdate {
                jobs: vec![job(4, 10)],
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        "LRR-E-DISCOVERY-LIMIT"
    );
    assert_eq!(next(&mut w), t);
    assert!(
        w.complete(
            &t,
            DiscoveryUpdate {
                dependencies: vec![(id(8), id(9)), (id(8), id(10))],
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(next(&mut w), t);
    w.complete(&t, DiscoveryUpdate::default()).unwrap();
    assert!(matches!(w.poll().unwrap(), DiscoveryStep::Sealed(_)));
}
#[test]
fn discovery_cycle_stays_incomplete_without_treating_unexpanded_producer_as_empty() {
    let mut w = work(Default::default());
    w.enqueue(job(3, 0)).unwrap();
    w.enqueue(job(4, 10)).unwrap();
    let t = next(&mut w);
    w.block(&t, vec![id(4)]).unwrap();
    let t = next(&mut w);
    w.block(&t, vec![id(3)]).unwrap();
    for _ in 0..2 {
        let DiscoveryStep::Incomplete { blocked, .. } = w.poll().unwrap() else {
            panic!("cycle falsely sealed")
        };
        assert_eq!(blocked, vec![(id(3), vec![id(4)]), (id(4), vec![id(3)])]);
    }
}
#[test]
fn discovery_one_invocation_can_advance_through_resolution_expansion_and_integration() {
    let mut w = work(Default::default());
    let mut j = job(3, 0);
    w.enqueue(j.clone()).unwrap();
    let t = next(&mut w);
    j.kind = DiscoveryKind::Expand;
    w.complete(
        &t,
        DiscoveryUpdate {
            jobs: vec![j.clone()],
            ..Default::default()
        },
    )
    .unwrap();
    let t = next(&mut w);
    assert_eq!(t.job().kind, DiscoveryKind::Expand);
    j.kind = DiscoveryKind::Integrate;
    w.complete(
        &t,
        DiscoveryUpdate {
            jobs: vec![j],
            ..Default::default()
        },
    )
    .unwrap();
    let t = next(&mut w);
    assert_eq!(t.job().kind, DiscoveryKind::Integrate);
    w.complete(
        &t,
        DiscoveryUpdate {
            members: vec![id(8)],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(w.poll().unwrap(), DiscoveryStep::Sealed(_)));
}
#[test]
fn discovery_seal_records_scanned_sources_even_when_no_names_contribute() {
    let mut seals = vec![];
    for n in [3, 4] {
        let mut w = work(Default::default());
        w.enqueue(job(n, 0)).unwrap();
        let t = next(&mut w);
        w.complete(&t, DiscoveryUpdate::default()).unwrap();
        let DiscoveryStep::Sealed(s) = w.poll().unwrap() else {
            panic!("seal absent")
        };
        seals.push(s.fingerprint());
    }
    assert_ne!(seals[0], seals[1]);
}
#[test]
fn discovery_seal_domain_epoch_and_unit_graph_are_distinct() {
    let mut identities = std::collections::BTreeSet::new();
    for domain in [
        SealDomain::ImplCatalog { id: id(1) },
        SealDomain::TestCatalog { id: id(1) },
        SealDomain::Scope {
            id: id(1),
            namespace: lrr::source::manifest::Namespace::Type,
        },
    ] {
        for graph in [id(2), id(3)] {
            for epoch in [1, 2] {
                let mut w =
                    Discovery::new(domain.clone(), graph, epoch, Default::default()).unwrap();
                let DiscoveryStep::Sealed(s) = w.poll().unwrap() else {
                    panic!("empty catalog absent")
                };
                assert!(identities.insert(s.fingerprint()));
            }
        }
    }
    assert_eq!(identities.len(), 12);
}
#[test]
fn discovery_large_ready_queue_advances_without_replaying_completed_prefixes() {
    let mut w = work(Default::default());
    for n in (0_u32..10000).rev() {
        let mut identity = [0; 32];
        identity[..4].copy_from_slice(&n.to_le_bytes());
        w.enqueue(DiscoveryJob {
            module_order: 0,
            source_offset: n,
            kind: DiscoveryKind::RegisterHeader,
            identity: Digest(identity),
        })
        .unwrap();
    }
    for n in 0_u32..10000 {
        let t = next(&mut w);
        assert_eq!(t.job().source_offset, n);
        w.complete(&t, DiscoveryUpdate::default()).unwrap();
    }
    assert!(matches!(w.poll().unwrap(), DiscoveryStep::Sealed(_)));
}

#[test]
fn discovery_completion_ticket_cannot_cross_unit_graph_requests() {
    let mut a = work(Default::default());
    let mut b = Discovery::new(
        SealDomain::TestCatalog { id: id(1) },
        id(9),
        1,
        Default::default(),
    )
    .unwrap();
    a.enqueue(job(3, 0)).unwrap();
    b.enqueue(job(3, 0)).unwrap();
    let ta = next(&mut a);
    let tb = next(&mut b);
    assert!(b.complete(&ta, DiscoveryUpdate::default()).is_err());
    assert_eq!(next(&mut b), tb);
    b.complete(&tb, DiscoveryUpdate::default()).unwrap();
}
