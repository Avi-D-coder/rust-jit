fn t(x: i32) -> i32 { let x = &x; let x = *x + 1; x }
fn main() { assert_eq!(t(3), 4); }
