fn t(x: i32) -> usize { let x = x.to_string(); x.len() }
fn main() { assert_eq!(t(12345), 5); }
