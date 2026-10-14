#![recursion_limit = "4"]
// Standalone native oracle; never a selected LRR live unit.
macro_rules! again { ($value:literal) => { again!($value) }; }
fn main() { let _ = again!(0); }
