fn t(foo: i32) -> i32 { let foo = foo + 1; macro_rules! g { () => { foo } } g!() }
fn main() { assert_eq!(t(3), 4); }
