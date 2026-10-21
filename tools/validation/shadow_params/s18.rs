fn t(n: i32) -> i32 { let mut i = 0; let mut acc = 0; while i < 3 { let n = n + i; acc += n; i += 1; } acc + n }
fn main() { assert_eq!(t(10), 43); }
