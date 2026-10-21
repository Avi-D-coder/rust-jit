fn t(p: (u8, u8)) -> i64 { let p = (5i64, 600i64); p.0 + p.1 }
fn main() { assert_eq!(t((1, 2)), 605); }
