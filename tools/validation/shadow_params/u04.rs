fn t(f: impl Fn(i32) -> i32) -> i32 { let f = move |x| f(x) + 1; f(1) }
fn main() { assert_eq!(t(|x| x * 10), 11); }
