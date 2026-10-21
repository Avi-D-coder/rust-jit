fn t(foo: i32) -> i32 { let r = if foo > 1 { let foo = foo * 2; foo } else { foo }; r + foo }
fn main() { assert_eq!(t(3), 9); assert_eq!(t(1), 2); }
