# M007 macro owner routing

`HygieneFacts::dollar_crate_bucket` projects the emitting definition's owned
context into the existing ordinary source `BucketKey`. It preserves the explicit
namespace, roots the scope at the definition unit, and bounds path depth and
aggregate UTF-8 bytes before constructing the key. The route never consults a
consumer dependency alias, the reexporting unit or a normalized call-site owner.
The source database still supplies candidates, completeness blockers and headers;
visibility and final reached-item resolution integrate with the later resolver.

`#[macro_export]` on `macro_rules!` now has an explicit `MacroExport` attribute
class with stable tag 9. Its tokens remain in the macro header. This known compiler
attribute is not an unknown procedural transformation and does not block the
provider's ordinary helper bucket. The before-classification fixture fails in
`artifacts/macos-first/m007-dollar-crate-before-export-facet.log`. Other attributes
and targets retain their existing classification. Generated/root export insertion,
macro_export option interpretation and the full source discovery queue still
require their later integration checks.

The native acceptance creates three real Cargo packages: the origin library,
a facade and a caller. Both the caller and facade rename the origin dependency;
the facade reexports its macro under another name. All three crates have a
`helper`. Rust 1.99.0 on macOS executes the direct and forwarded macro, proving
both select the origin helper. The LRR comparison uses actual pinned marked
tokens, owned hygiene transport, dependency/reexport namespace facts and the
existing prepared source publication/database to select the same helper identity.
Selected identities map to values authored into that fixture's real source.
The fixture explicitly supplies unit/import facts: it does not claim Cargo source
collection, final guest VM execution, visibility coverage or full M007 completion.
