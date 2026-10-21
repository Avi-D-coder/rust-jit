fn t(x: u8) -> (i64, i64) { let x = 4000i64; (x, x) }
fn main() { assert_eq!(t(1), (4000, 4000)); }
