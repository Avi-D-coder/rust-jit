fn t(f: impl Fn(i32) -> i32) -> i32 { let f = f(1); f + 1 }
fn u(g: i32) -> i32 { let g = |x: i32| x + g; g(1) }
fn main() { assert_eq!(t(|x| x * 10), 11); assert_eq!(u(5), 6); }
