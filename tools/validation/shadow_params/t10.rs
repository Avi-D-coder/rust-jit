fn t(x: u8) -> bool { let x = true; !x }
fn u(x: u8) -> i64 { let x = 1000i64; -x }
fn v(x: u8) -> i64 { let x = 1000i64; x as i64 }
fn main() { assert!(!t(1)); assert_eq!(u(1), -1000); assert_eq!(v(1), 1000); }
