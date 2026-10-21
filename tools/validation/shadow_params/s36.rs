fn t(x: i32) -> i32 { let x = x as i64; (x * 2) as i32 }
fn main() { assert_eq!(t(3), 6); }
