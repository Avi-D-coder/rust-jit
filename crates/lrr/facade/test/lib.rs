// Partial LRR source facade for the `test` crate. The export follows pinned
// rustc library/test/src/lib.rs; Bencher and the libtest runtime are separate
// library work and are not provided here.
extern crate std;

mod bench;

pub use self::bench::black_box;
