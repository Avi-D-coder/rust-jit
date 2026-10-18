//! LRR route/exact-once control supplementing the unchanged rustc enum-field
//! original. It checks each nested goal and repeated generic activation,
//! rather than using the transaction root or source name as a Type cache key.
use super::StandaloneOutcome;

#[test]
fn nested_nominal_pattern_goals_keep_each_actual_application_and_scrutinee_once() {
    const SOURCE: &str = r#"
        trait Project { type Value; }
        impl Project for u8 { type Value = u32; }
        impl Project for u32 { type Value = u8; }
        struct W<T: Project> { marker: T, value: T::Value }
        fn make(calls: &mut u32) -> (W<u8>, W<u32>) {
            *calls += 1;
            (W::<u8> { marker: 1_u8, value: 33_u32 },
             W::<u32> { marker: 2_u32, value: 44_u8 })
        }
        fn take<T: Project>(value: W<T>) -> bool {
            match value { W { value: _, .. } => true }
        }
        macro_rules! consume {
            ($value:expr) => {
                match $value {
                    (W { value: a, .. }, W { value: b, .. }) =>
                        a == 33_u32 && b == 44_u8,
                }
            };
        }
        macro_rules! wrapper { ($value:expr) => { consume!($value) }; }
        fn main() {
            let mut calls = 0_u32;
            let ordinary = match make(&mut calls) {
                (W { value: a, .. }, W { value: b, .. }) =>
                    a == 33_u32 && b == 44_u8,
            };
            assert!(ordinary);
            assert!(consume!(make(&mut calls)));
            assert!(wrapper!(make(&mut calls)));
            assert!(calls == 3_u32);
            assert!(take(W::<u8> { marker: 3_u8, value: 55_u32 }));
            assert!(take(W::<u32> { marker: 4_u32, value: 66_u8 }));
        }
    "#;
    match super::crate_graph::format_trait_facade_tests::run(
        "reached-pattern-nominal",
        SOURCE,
        2021,
    ) {
        Ok(StandaloneOutcome::Completed(report)) => {
            assert!(report.opcode_counters.started_by_opcode[45] > 0);
        }
        Ok(_) => panic!("reached pattern nominal route did not complete"),
        Err(error) => panic!("reached pattern nominal route: {error:?}"),
    }
}
