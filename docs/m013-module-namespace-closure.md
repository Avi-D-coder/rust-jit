# M013 requested module namespace closure

M013 remains in progress. Explicit namespace completion first requires fresh source
readiness and completed physical macro catalogue installation. It constructs actual
source and completed-generated macro routes with the retained inherited module
context. The namespace graph receives the child's real macro exports, import routes,
aliases and visibility through the existing route/import machinery. Route count and
retained metadata are bounded.

Retained physical module routes participate in every subsequent source discovery
macro namespace installation, including later original-source selection and final
revalidation. Completed generated catalogues use the same inherited context for
route construction as for actual macro demands. A child's macro_export definition
can therefore resolve a later original invocation without substituting a generated
macro body or inventing declaration origin metadata.

Only after successful route installation does completion retire that module's
actual parent-owned namespace frontier. The requested namespace's import fixed
point is recomputed and the module is marked namespace-complete. Its existing
ScanNestedDeclarations job then completes with the actual producer/projection
identities, waking blocked work. Unknown producers and unresolved imports refuse
readiness and cannot trigger this transition. Application failures poison discovery.
The other required namespace/trait/test/publication obligations remain separate.

Three controls exercise a real value namespace seal after proven module closure,
physical macro_export body selection for a later original call, and preservation
of actual unknown/missing-import obligations. Cold bodies remain lazy. Existing
macro_use, private module and recursive generated controls also pass.

Source publication currently refuses closed module inputs with
LRR-E-CATALOG-INCOMPLETE until schema-checked child input registration is available.
This explicit refusal prevents a successful root-only publication from claiming
child source rows exist. Database revision remains unchanged by namespace closure.
Actual child source/cache publication and guest execution remain mandatory next
integration work. The historical positive module native/LRR witness remains pending;
no successful child runtime equality is claimed.

Complete namespace facets, nested physical module layout/context, child compiler
routing, final source revalidation and full invalidation/quota integration remain
required. Full MX027/MX028, JIT, release and Linux acceptance are unfulfilled.
macOS ARM64 Rust 1.99.0 remains primary.
