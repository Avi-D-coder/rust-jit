fn t(f: impl Fn(i32) -> i32) -> i32 { let g = f; let f = |x: i32| g(x) * 2; f(3) }
fn main() { assert_eq!(t(|x| x + 1), 8); }
