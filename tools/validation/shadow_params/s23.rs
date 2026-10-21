fn t(x: String) -> usize { let x = x.len(); x }
fn u(x: String) -> String { let x = x + "!"; x }
fn main() { assert_eq!(t("abc".to_string()), 3); assert_eq!(u("a".to_string()), "a!"); }
