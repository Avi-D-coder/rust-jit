// Standalone native-oracle fixture; never a selected LRR live Cargo unit.
macro_rules! pairs {
    ($($a:literal),*; $($b:literal),*) => { [$(($a, $b)),*] };
}
fn main() { let _ = pairs!(1, 2; 3); }
