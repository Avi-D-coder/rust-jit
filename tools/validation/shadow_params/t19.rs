struct W { v: i64 }
fn t(x: u8) -> i64 { let x = W { v: 70000 }; x.v }
fn u(x: u8) -> i64 { let mut x = W { v: 70000 }; x.v += 1; x.v }
fn main() { assert_eq!(t(1), 70000); assert_eq!(u(1), 70001); }
