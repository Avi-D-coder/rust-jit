const K: i32 = 5;
fn t(k: i32) -> i32 { let k = K + k; k }
fn main() { assert_eq!(t(1), 6); }
