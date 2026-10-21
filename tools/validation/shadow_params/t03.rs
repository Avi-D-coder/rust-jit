fn t(a: u8) -> i64 { let a = 7i64; let r = &a; *r }
fn u(a: u8) -> i64 { let mut a = 7i64; { let r = &mut a; *r += 1000; } a }
fn main() { assert_eq!(t(1), 7); assert_eq!(u(2), 1007); }
