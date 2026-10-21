fn t(a: u8) -> i64 { let mut a = 100i64; a += 5; a }
fn u(a: u8) -> i64 { let mut a = 100i64; a *= 20; a -= 1; a }
fn main() { assert_eq!(t(1), 105); assert_eq!(u(2), 1999); }
