//! Source-only boundary controls alongside the unchanged rustc float-nan case.
use super::tests::{ORIGINAL, world};
use crate::{
    db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
    syntax::floating_paths::{qualified_floating_path, written_two_segment_path},
};
#[test]
fn stock_floating_path_adapter_rejects_reversed_outside_and_nonexact_ranges() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, _, body, _, _) = world(ORIGINAL, "main", edition);
        let owner = view
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|owner| owner.key() == body)
            .unwrap();
        let output = db
            .fetch(
                &SyntaxProvider,
                QueryKey::ParseFile(view.selection().outline.parse.clone()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::ParseFile(parsed) = output.value.as_ref() else {
            panic!("wrong parse result")
        };
        let start = u32::try_from(ORIGINAL.find("f64::NAN").unwrap()).unwrap();
        let end = start + 8;
        let path = written_two_segment_path(parsed, owner, start, end)
            .unwrap()
            .unwrap();
        assert_eq!(
            (path.qualifier.as_str(), path.member.as_str()),
            ("f64", "NAN")
        );
        for (a, b) in [(end, start), (0, end), (start, u32::MAX)] {
            assert_eq!(
                written_two_segment_path(parsed, owner, a, b)
                    .unwrap_err()
                    .code,
                "LRR-E-TYPE-CONTEXT"
            );
        }
        assert!(
            written_two_segment_path(parsed, owner, start + 1, end)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn floating_adapter_keeps_unknown_members_distinct_from_dispatch_admission() {
    let source = "fn main() { f64::UNRECOGNIZED; }";
    let (mut db, view, _, body, _, _) = world(source, "main", 2024);
    let owner = view
        .outline()
        .items
        .iter()
        .filter_map(|item| item.body.as_ref())
        .find(|owner| owner.key() == body)
        .unwrap();
    let output = db
        .fetch(
            &SyntaxProvider,
            QueryKey::ParseFile(view.selection().outline.parse.clone()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::ParseFile(parsed) = output.value.as_ref() else {
        panic!("wrong parse result")
    };
    let start = u32::try_from(source.find("f64::UNRECOGNIZED").unwrap()).unwrap();
    let end = start + 17;
    let path = written_two_segment_path(parsed, owner, start, end)
        .unwrap()
        .unwrap();
    assert_eq!(path.member, "UNRECOGNIZED");
    assert!(
        qualified_floating_path(parsed, owner, start, end)
            .unwrap()
            .is_none()
    );
}
#[test]
fn floating_adapter_rejects_generic_and_absolute_paths_without_erasing_source() {
    for path in ["f64::<u8>::NAN", "f64::NAN::<u8>", "::f64::NAN"] {
        let source = format!("fn main() {{ {path}; }}");
        let (mut db, view, _, body, _, _) = world(&source, "main", 2024);
        let owner = view
            .outline()
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|owner| owner.key() == body)
            .unwrap();
        let output = db
            .fetch(
                &SyntaxProvider,
                QueryKey::ParseFile(view.selection().outline.parse.clone()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::ParseFile(parsed) = output.value.as_ref() else {
            panic!("wrong parse result")
        };
        let start = u32::try_from(source.find(path).unwrap()).unwrap();
        let end = start + u32::try_from(path.len()).unwrap();
        assert!(
            written_two_segment_path(parsed, owner, start, end)
                .unwrap()
                .is_none(),
            "unexpected path admission: {path}"
        );
        assert!(
            qualified_floating_path(parsed, owner, start, end)
                .unwrap()
                .is_none()
        );
    }
}
