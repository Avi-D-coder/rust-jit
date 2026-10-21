fn t((a, b): (i32, i32)) -> i32 { let a = a + b; a * 10 + b }
fn main() { assert_eq!(t((3, 4)), 74); }
