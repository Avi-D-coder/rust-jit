fn f(x: i32) -> i32 { x * 1000 }
fn t(f: i32) -> i32 { let f = f + 1; f }
fn main() { assert_eq!(t(3), 4); assert_eq!(f(2), 2000); }
