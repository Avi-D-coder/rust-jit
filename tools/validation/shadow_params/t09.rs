fn t(x: u8) -> i64 { let x = 9i64; return x; }
fn id(v: i64) -> i64 { v }
fn u(x: u8) -> i64 { let x = 9i64; id(x) }
fn main() { assert_eq!(t(1), 9); assert_eq!(u(1), 9); }
