use lrr::{
    core::digest::Digest,
    expand::declarative::{DeclarativeSession, ExpansionLimits},
    syntax::token_wire::*,
};
fn output() -> TokenForest {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let span = SpanRecord {
        source: SourceIdentity {
            unit: Key([1; 32]),
            content: Key([2; 32]),
            path: "generated.rs".into(),
        },
        start: 0,
        end: 1,
        context: context.key(),
        edition: Edition::E2024,
        origin: Key([3; 32]),
    };
    TokenForest::new(
        vec![TokenTree::Literal {
            token_text: "1".into(),
            span: span.key(),
        }],
        vec![context],
        vec![span],
    )
    .unwrap()
}
#[test]
fn declaration_session_checks_parent_order_and_poisoned_request_without_reusing_output() {
    let mut session = DeclarativeSession::new(ExpansionLimits::default()).unwrap();
    let root = session.begin(None, Digest([1; 32])).unwrap();
    assert!(session.begin(Some(root), Digest([2; 32])).is_err());
    assert!(session.adopt(root, &output()).is_err());
    let mut fresh = DeclarativeSession::new(ExpansionLimits::default()).unwrap();
    let root = fresh.begin(None, Digest([1; 32])).unwrap();
    fresh.adopt(root, &output()).unwrap();
    let child = fresh.begin(Some(root), Digest([2; 32])).unwrap();
    fresh.adopt(child, &output()).unwrap();
    assert!(fresh.finish(root).is_err());
    assert!(fresh.finish(child).is_err());
}
#[test]
fn declaration_session_enforces_reduced_token_byte_and_invocation_limits_atomically() {
    for limits in [
        ExpansionLimits {
            generated_tokens: 1,
            ..ExpansionLimits::default()
        },
        ExpansionLimits {
            cumulative_output_bytes: output().encode().unwrap().len() as u64,
            ..ExpansionLimits::default()
        },
        ExpansionLimits {
            invocations: 1,
            ..ExpansionLimits::default()
        },
    ] {
        let mut session = DeclarativeSession::new(limits).unwrap();
        let first = session.begin(None, Digest([1; 32])).unwrap();
        session.adopt(first, &output()).unwrap();
        session.finish(first).unwrap();
        let before = session.counts();
        match session.begin(None, Digest([2; 32])) {
            Err(error) => assert_eq!(error.code, "LRR-E-MACRO-OUTPUT"),
            Ok(next) => {
                assert_eq!(
                    session.adopt(next, &output()).unwrap_err().code,
                    "LRR-E-MACRO-OUTPUT"
                );
                assert_eq!(session.counts().generated_bytes, before.generated_bytes);
                assert_eq!(session.counts().generated_tokens, before.generated_tokens);
            }
        }
        assert!(session.begin(None, Digest([3; 32])).is_err());
    }
    assert!(
        DeclarativeSession::new(ExpansionLimits {
            frames: 257,
            ..ExpansionLimits::default()
        })
        .is_err()
    );
}

#[test]
fn declaration_adoption_measures_exact_bytes_before_committing_reduced_output_budget() {
    let candidate = output();
    let exact = candidate.encoded_len().unwrap() as u64;
    assert_eq!(exact, candidate.encode().unwrap().len() as u64);
    for (limit, accepted) in [(exact - 1, false), (exact, true)] {
        let mut session = DeclarativeSession::new(ExpansionLimits {
            single_output_bytes: limit,
            cumulative_output_bytes: exact,
            ..ExpansionLimits::default()
        })
        .unwrap();
        let frame = session.begin(None, Digest([5; 32])).unwrap();
        let result = session.adopt(frame, &candidate);
        if accepted {
            result.unwrap();
            assert_eq!(session.counts().generated_bytes, exact);
            session.finish(frame).unwrap();
        } else {
            assert_eq!(result.unwrap_err().code, "LRR-E-MACRO-OUTPUT");
            assert_eq!(session.counts().generated_bytes, 0);
            assert_eq!(session.counts().generated_tokens, 0);
            assert!(session.finish(frame).is_err());
            assert!(session.adopt(frame, &candidate).is_err());
        }
    }
}
