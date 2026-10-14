// Standalone native-oracle fixture; production LRR live units are never compiled.
use m006_edition2021::older_expr;
macro_rules! newer_expr {
    ($value:expr) => { 1 };
    (const $value:block) => { 2 };
}
macro_rules! preserved_expr {
    ($value:expr_2021) => { 1 };
    (const $value:block) => { 2 };
}
macro_rules! values { ($($value:literal),*) => { [$($value),*] }; }
macro_rules! nested_values { ($($value:literal),*) => { values!($($value),*) }; }
macro_rules! ragged {
    ($(($($a:literal),*; $($b:literal),*)),*) => { ($([$(( $a, $b )),*]),*) };
}
fn main() {
    println!("{:?}", [older_expr!(const { 7 }), newer_expr!(const { 7 }), preserved_expr!(const { 7 })]);
    println!("{:?}", values!(7, 2, 11));
    let rows = ragged!((1,2;11,12),(3;13));
    println!("{:?}", [rows.0[0].0, rows.0[0].1, rows.0[1].0, rows.0[1].1, rows.1[0].0, rows.1[0].1]);
    println!("{:?}", nested_values!(7,2,11));
}
