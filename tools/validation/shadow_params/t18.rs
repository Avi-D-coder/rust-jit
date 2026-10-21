fn t(x: u8) -> i64 { let x = 70000i64; match x { 70000 => x, _ => 0 } }
fn main() { assert_eq!(t(1), 70000); }
