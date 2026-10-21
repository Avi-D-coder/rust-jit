fn t(v: Vec<i32>) -> Vec<i32> { let mut v = v; v.push(1); v }
fn main() { assert_eq!(t(vec![0]), vec![0, 1]); }
