fn t(x: i32) -> i32 { match x { x if x > 2 => x * 2, x => x } }
fn main() { assert_eq!(t(3), 6); assert_eq!(t(1), 1); }
