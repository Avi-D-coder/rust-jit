fn t(a: i32, b: i32) -> i32 { let (a, b) = (b, a); a * 10 + b }
fn main() { assert_eq!(t(1, 2), 21); }
