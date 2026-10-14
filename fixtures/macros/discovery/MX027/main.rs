#[macro_export]
macro_rules! origin {
    () => {
        pub fn answer() -> i64 { 41_i64 }
    };
}
macro_rules! bootstrap {
    () => {
        use crate::origin as imported;
        macro_rules! later { () => { imported!(); }; }
    };
}
bootstrap!();
later!();
fn check() -> bool { answer() == 41_i64 }
fn main() { assert!(check()); }
