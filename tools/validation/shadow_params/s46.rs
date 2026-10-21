fn t(a: i32) -> i32 { let a = a; let a = { let a = a + 1; a * 2 }; a }
fn main() { assert_eq!(t(1), 4); }
