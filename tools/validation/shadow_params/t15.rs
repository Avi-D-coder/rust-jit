fn t(x: u8) -> Vec<i64> { let x = 70000i64; vec![x, x + 1] }
fn main() { assert_eq!(t(1), vec![70000, 70001]); }
