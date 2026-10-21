struct P { x: i32, y: i32 }
fn t(p: P) -> i32 { let P { x, y } = p; let x = x + y; x }
fn u(x: P) -> i32 { let P { x, y } = x; x * y }
fn main() { assert_eq!(t(P { x: 1, y: 2 }), 3); assert_eq!(u(P { x: 3, y: 4 }), 12); }
