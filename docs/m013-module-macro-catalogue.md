# M013 physical source macro catalogue installation

M013 remains in progress. Explicit catalogue installation requires a fresh positive
ModuleSourceReadiness result and no active discovery ticket. It retains a bounded
immutable child SourceDeclarative snapshot and verifies the actual module parent
catalogue before admitting completed child root bindings. Unknown source producers
remain pending; catalogue installation does not publish database inputs or release
the file-module namespace frontier.

The existing generated textual scope placeholder is completed with the physical
child's real root bindings. Existing macro_use exports then make those bindings
visible at the module declaration's export point. A plain module keeps them in its
child scope. Hygiene facts, declaration metadata and expansion ancestry are copied
through the existing owned resolver. Physical declaration origins are optional,
and remain absent when the actual source declaration has no expansion origin;
generated declarations retain their actual origin.

Physical macro bodies are retained separately from GeneratedItems bodies. Demand
selects an actual source owner and prepares its exact body/input request. Explicit
expansion delegates to that owner's immutable source or generated body. Generated
child completion transfers physical body ownership rather than substituting the
parent's generated tokens. Catalogue expansion and string operands preserve the
same body selection rule.

A completed module's declaration cutoff/context remains retained in the catalogue.
Completed original-source catalogues and their actual selection children can be
looked up after the discovery fragment finishes, with their original invocation
position and bounded ancestor order. Readiness and catalogue replay can therefore
revalidate against the actual completed parent. Catalogue growth and retained
source/index metadata remain checked; application failure poisons discovery.

Four controls inspect actual pinned tokens for a macro_use call in generated
parent output and a later original call, readiness replay after parent completion,
unknown source refusal, and completed plain-module macro privacy. They also verify
that no database revision/publication or final module frontier release occurs.

Macro-export namespace facets, complete import facets, derived source/cache input
publication, final selection revalidation/frontier retirement, nested physical
module layout/context, child compiler routing and actual LRR child execution remain
required. This is not full module/MX027/MX028 or native/guest runtime acceptance.
The historical positive module native/LRR witness remains pending. Full JIT,
release and Linux acceptance remain unfulfilled. macOS ARM64 Rust 1.99.0 is primary.
