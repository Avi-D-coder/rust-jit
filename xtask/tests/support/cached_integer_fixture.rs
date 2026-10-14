//! Test-only migration from genuine fixture spelling to production cached metadata.
//! Uses the ordinary captured-source/reached-thunk path; no alternate lexer/parser.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{
        op::{DecodedLiteral, LiteralKind, Op},
        pc::Phase,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
};
pub fn integer(spelling: &str) -> DecodedLiteral {
    let input = SourceUnitInput {
        path: SourcePath::new("cached_integer_fixture.rs").unwrap(),
        crate_key: Digest([250; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(format!("fn __lrr_integer_fixture() {{ {spelling}; }}").into_bytes())
            .unwrap(),
    };
    let selection = SourceSelection {
        path: input.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: input.blob.content(),
                profile: input.profile.clone(),
            },
            crate_key: input.crate_key,
            module_path: vec![],
            cfg: input.cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(None, vec![input], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("literal fixture source absent")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "__lrr_integer_fixture")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(indexed)) =
        db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key()))
    else {
        panic!("literal fixture structure incomplete")
    };
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!("literal fixture body variant differs")
    };
    let mut literals = index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Literal));
    let node = literals
        .next()
        .expect("fixture must contain its genuine integer literal");
    assert!(
        literals.next().is_none(),
        "fixture must contain exactly one literal"
    );
    let QueryPoll::Ready(Ok(lowered)) = db.fetch(
        &SyntaxProvider,
        QueryKey::Thunk(index.pc(node.id, Phase::Entry).unwrap()),
    ) else {
        panic!("literal fixture recipe incomplete")
    };
    let QueryValue::Thunk(recipe) = lowered.value.as_ref() else {
        panic!("literal fixture recipe variant differs")
    };
    let mut decoded = recipe.recipe().operations.iter().filter_map(|op| {
        let Op::Literal { literal, .. } = op else {
            return None;
        };
        assert_eq!(literal.kind, LiteralKind::Integer);
        assert_eq!(literal.spelling, spelling);
        assert_eq!(literal.source, index.span(node.id).unwrap());
        Some(
            literal
                .decoded
                .as_ref()
                .expect("current integer schema requires production decoded metadata")
                .clone(),
        )
    });
    let result = decoded
        .next()
        .expect("reached recipe must contain integer metadata");
    assert!(decoded.next().is_none());
    assert!(matches!(
        result,
        DecodedLiteral::Integer { .. } | DecodedLiteral::InvalidInteger { .. }
    ));
    result
}
