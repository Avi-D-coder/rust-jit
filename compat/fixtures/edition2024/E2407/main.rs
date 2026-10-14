struct Probe(&'static str);
impl Drop for Probe { fn drop(&mut self) { println!("drop:{}", self.0); } }
impl Probe { fn value(&self) -> i64 { 7_i64 } }
enum Choice { Yes(Probe), No(Probe) }
struct Holder<'a> { probe: &'a Probe }
fn iteration(n: i64) -> Choice { println!("evaluate:{}", n); Choice::Yes(Probe(if n == 0_i64 { "iteration0" } else { "iteration1" })) }
#[test]
fn case() {
let mut n: i64 = 0_i64;
while let Choice::Yes(ref probe) = iteration(n) && n < 4_i64 {
println!("body:{}", probe.0); n = n + 1_i64;
if n == 1_i64 { println!("continue"); continue; }
println!("break"); break;
}
println!("after_while");
}
