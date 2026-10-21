fn t<T: Clone>(foo: T) -> T { let foo = foo.clone(); foo }
fn main() { assert_eq!(t(7u8), 7u8); assert_eq!(t(String::from("a")), "a"); }
