fn t(foo: std::ops::Range<i32>) -> i32 { let mut s = 0; for foo in foo { s += foo; } s }
fn main() { assert_eq!(t(0..5), 10); }
