fn t(x: u8) -> i64 { let x = 1_000_000i64; let y = x; y + x }
fn main() { assert_eq!(t(1), 2_000_000); }
