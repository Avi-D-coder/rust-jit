fn t(foo: i32) -> i32 { macro_rules! g { () => { foo } } let foo = 100; g!() + foo * 0 }
fn main() { assert_eq!(t(3), 3); }
