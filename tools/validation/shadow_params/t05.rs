fn t(i: u8, v: Vec<i32>) -> i32 { let i = 1usize; v[i] }
fn main() { assert_eq!(t(0, vec![5, 6, 7]), 6); }
