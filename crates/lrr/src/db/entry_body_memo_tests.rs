//! Entry service route control over an unchanged official Rust source.
use super::*;
use crate::{db::trace::Action, frontend::callables::CallSelection};

#[test]
fn actual_standalone_entry_reuses_selected_body_memo_all_editions() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, discovery) = setup(text, edition);
        db.begin_expansion_epoch(1).unwrap();
        discovery.publish(&mut db).unwrap();
        let CallSelection::Source(callable) = ready(discovery.entry_function(&mut db, "main"))
        else {
            panic!("actual original main did not select a source function");
        };
        let definition = callable.identity();
        let first =
            crate::cli::retained_entry::prepare(&mut db, &discovery, &source, "main", callable)
                .unwrap();
        let body = first.body().unwrap().key();
        assert_eq!(first.identity(), definition);
        let second =
            crate::cli::retained_entry::prepare(&mut db, &discovery, &source, "main", first)
                .unwrap();
        second.revalidate(&mut db).unwrap();
        assert_eq!(second.identity(), definition);
        assert_eq!(second.body().unwrap().key(), body);
        assert_eq!(
            db.trace
                .events
                .iter()
                .filter(|event| {
                    matches!(event.key, QueryKey::PreparedBodySyntax(_))
                        && event.action == Action::ProviderRun
                })
                .count(),
            1,
            "the actual entry adapter must reuse the immutable selected assembly"
        );
        assert_eq!(db.trace.truncated, 0);
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| { !matches!(event.key, QueryKey::Thunk(_)) }),
            "entry preparation must not lower executable phases"
        );
    }
}
