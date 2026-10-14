// M007 native witness: an expanded name cannot silently capture the less
// expanded outer name. Full LRR hygiene integration must reproduce E0659.
macro_rules! choose { () => {29}; }
macro_rules! generate { () => { macro_rules! choose { () => {41}; } }; }
fn main() {
    generate!();
    let _ = choose!();
}
