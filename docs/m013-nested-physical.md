# M013: nested generated physical module adoption

Revision 4 / Section 57 milestone C, macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: each generated external module declaration binds to its actual
adopted physical caller, retained capture directory, source publication and
lexical macro lineage. The original root ordinal remains deterministic ordering
metadata; it cannot substitute for a physical source identity.

Generated expansion records from staged physical modules now retain their exact
source-module producer in a deterministic bounded record index. Descendant
expansions inherit that producer. Caller binding and layout validation use the
retained staged syntax instead of selecting the original root by ordinal. The
explicit coordinator uses the parent's captured directory. A nested caller is
validated against its adopted ModuleRecord and exact staged SourceSelection;
original callers still require their captured source publication.

Module scope lookup follows the retained physical parent chain with a checked
256-frame bound. It supports completed staged catalogues as well as active
fragments. Inherited contexts flatten the actual outer scopes and generated body
owners, retain captured source body owners, and combine hygiene/ancestry within
the existing metadata limits. This preserves each physical and original source
cutoff without inventing a new frontend or executing a query-provider effect.

Three CLI/native comparisons cover generated grandchild capture with a competing
root file, three adopted file levels with competing directories, and an original
root macro shadowed later in both root and child. Each compares real guest
execution with an explicitly compiled isolated Rust 1.99.0 macOS oracle. No
selected live runtime unit is compiled. The pre-change source-base refusal is
retained. These controls do not establish arbitrary nested source combinations
or complete depth/quota boundaries.

Broader ordinary/generated/included contexts, full quota boundary coverage,
service/facets/cache/runtime/type integration, full MX027/MX028, JIT/release and
Linux remain required. M014–M016 add nested declaration producer/catalogue
requirements in fixed order. Checkpoint fields report only observed gate/audit
results, with M013 in_progress and release_complete=false.
