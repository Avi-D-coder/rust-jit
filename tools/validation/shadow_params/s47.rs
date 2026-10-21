fn t(x: i32) -> i32 { let mut r = 0; for x in 0..x { let x = x * 2; r += x; } r + x }
fn main() { assert_eq!(t(3), 6 + 3); }
