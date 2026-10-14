pub fn cold() { lrr_deliberately_missing_two_output_function(); }
#[test]
fn distinct_unit_includes() { assert_eq!(lrr_two_output_left::VALUE, 31); assert_eq!(lrr_two_output_right::VALUE, 47); }
