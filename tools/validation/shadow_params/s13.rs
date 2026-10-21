fn t(mut x: i32) -> i32 { x += 1; let x = x * 2; x }
fn main() { assert_eq!(t(3), 8); }
