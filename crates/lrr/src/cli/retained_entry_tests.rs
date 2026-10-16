//! Unchanged stock runtime main through the actual standalone entry service.
use super::super::*;
const ORIGINAL: &[u8] =
    include_bytes!("../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/issue-40847.rs");

#[test]
fn unchanged_nested_macro_caller_binding_runs_the_whole_main_without_assertion_recipes() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/issue-40847.rs"
    ));
    assert_eq!(std::fs::read(path).unwrap(), ORIGINAL);
    for edition in [2015, 2018, 2021, 2024] {
        let report = match standalone_edition(path, None, edition) {
            Ok(StandaloneOutcome::Completed(report)) => report,
            Ok(_) => panic!("unchanged nested macro main did not complete: edition={edition}"),
            Err(error) => {
                panic!("unchanged nested macro main failed: edition={edition}, error={error:?}")
            }
        };
        assert_eq!(report.status, "completed");
        assert!(matches!(
            report.value,
            crate::engine::run::ValueSummary::Unit
        ));
        assert_eq!(
            report.opcode_counters.started_by_opcode[26], 0,
            "actual standard assertion must use ordinary expanded source, edition={edition}"
        );
        // The pinned actual core definition matches a tuple of shared borrows.
        assert!(report.opcode_counters.started_by_opcode[45] > 0);
        assert!(report.opcode_counters.started_by_opcode[31] > 0);
        assert!(report.opcode_counters.started_by_opcode[8] > 0);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert!(report.opcode_counters.return_scopes_cleaned > 0);
    }
    assert_eq!(std::fs::read(path).unwrap(), ORIGINAL);
}
