fn t(x: i32) -> i32 { let x = x; let y = x; let x = y + 1; x }
fn main() { assert_eq!(t(1), 2); }
