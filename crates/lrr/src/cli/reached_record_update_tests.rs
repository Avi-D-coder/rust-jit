//! Route/exact-once/ownership control supplementing unchanged stock FRU tests.
//! The omitted array forces the existing iterative Copy continuation to resume.
use super::StandaloneOutcome;

#[test]
fn record_updates_share_place_transfers_and_resumable_copy_all_editions() {
    const SOURCE: &str = r#"
        struct Token(u32);
        struct R { overwritten: Token, kept: [u32; 300], moved: Token }
        fn explicit(log: &mut u32) -> Token {
            *log = *log * 10_u32 + 1_u32;
            Token(9_u32)
        }
        fn base(log: &mut u32) -> R {
            *log = *log * 10_u32 + 2_u32;
            R { overwritten: Token(1_u32), kept: [5_u32; 300], moved: Token(6_u32) }
        }
        fn check(value: R) -> bool {
            value.overwritten.0 == 9_u32 && value.kept[299_usize] == 5_u32 && value.moved.0 == 6_u32
        }
        macro_rules! update {
            ($log:expr) => { R { overwritten: explicit($log), ..base($log) } };
        }
        macro_rules! wrapper { ($log:expr) => { update!($log) }; }
        fn main() {
            let mut log = 0_u32;
            let ordinary = R { overwritten: explicit(&mut log), ..base(&mut log) };
            assert!(check(ordinary));
            assert!(check(update!(&mut log)));
            assert!(check(wrapper!(&mut log)));
            assert!(log == 121212_u32);
            let original = R { overwritten: Token(1_u32), kept: [5_u32; 300], moved: Token(6_u32) };
            let updated = R { overwritten: Token(9_u32), ..original };
            assert!(original.overwritten.0 == 1_u32);
            assert!(original.kept[299_usize] == 5_u32);
            assert!(check(updated));
        }
    "#;
    for edition in [2015, 2018, 2021, 2024] {
        match super::crate_graph::format_trait_facade_tests::run(
            "record-update-place-copy",
            SOURCE,
            edition,
        ) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert!(
                    report.opcode_counters.started_by_opcode
                        [usize::from(crate::ir::op::CONSTRUCT_REST_OPCODE)]
                        >= 4
                );
            }
            Ok(_) => panic!("{edition}: record update route did not complete"),
            Err(error) => panic!("{edition}: record update route: {error:?}"),
        }
    }
}
