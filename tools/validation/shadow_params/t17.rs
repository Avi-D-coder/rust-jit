fn t(x: u8) -> i64 { let x = 70000i64; if x > 5 { x } else { 0 } }
fn main() { assert_eq!(t(1), 70000); }
