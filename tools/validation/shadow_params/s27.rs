fn t(foo: i32) -> i32 { let c = move |foo: i32| { let foo = foo + 1; foo }; c(foo) }
fn main() { assert_eq!(t(3), 4); }
