# M013 generated recursion inside physical file modules

M013 remains in progress. Deferred generated output from a physical child now
uses the existing GeneratedFragment worklist and its actual generated child
catalogues. ResolveMacro still returns a physical NeedExpansion request. After
admission, ScanNestedDeclarations returns real NeedGeneratedExpansion requests;
expansion remains external. Exact generated reply checks, record retention,
header/import plans, child parking/completion, quotas and dependencies reuse the
existing discovery paths.

A generated fragment identifies its physical source module separately from the
original source owner ordinal. A bounded inherited lexical context joins that
module's retained generated parent scope at its declaration cutoff, actual generated
ancestors at each invocation position, and the original captured parent scopes.
The existing hygienic resolver receives those outer scopes, owned hygiene facts
and expansion ancestry. Generated body selection and textual/path collision
checks also consult the actual inherited definition owner and origin metadata.
No catalogue is substituted solely on a runtime name or a synthetic source offset.

Completed generated recursion publishes its actual bindings into the retained
physical child's macro index. Only then does it complete that physical invocation
and let a later call resume. Its shared source frontier closes only when every
actual call sharing it has completed. The outer file-module frontier remains open.
Generated release identities are not retained as unrelated opaque producers.

A parent generated discovery job may still be active after output admission.
Source staging now checks that condition before namespace/source/job mutation.
It refuses with LRR-E-DISCOVERY-TICKET, leaves the child unstaged, and can retry
after normal polling blocks the parent on its unresolved module. This avoids
poisoning discovery with a partially installed child.

Three controls exercise two levels of generated recursion; child-local,
original-parent and generated-parent body ownership; parent and ancestor textual
cutoffs; exact reply refusal/replay; completing recursion before a later physical
call resumes; and safe active-ticket staging refusal followed by successful retry.
They inspect actual pinned expansion tokens and discovery state. They do not
claim guest execution or a complete module LRR/native equality fixture.

Nested physical file-module layout/context, physical-child compiler macro routing,
complete namespace imports/exports, child selection revalidation/source-cache
publication and final module-frontier closure remain required. Unsupported nested
physical module/compiler contexts retain typed refusal rather than selecting an
original-source context. Full MX027/MX028, JIT, release and Linux acceptance remain
unfulfilled; the historical positive module runtime witness remains pending.
macOS ARM64 Rust 1.99.0 is the primary validation platform.
