# M013 Rust 2015 dependency source and extern routes

M013 remains in progress. Explicit source preparation admits supported older editions
only for recorded dependency providers. The live entry profile remains Rust 2024.
Source selection checks the publication's recorded dependency role before accepting
an older profile; unknown editions and incompatible semantics fail closed.

Rust 2015 use declarations do not resolve the implicit extern prelude; qualified
macro invocations may resolve a recorded extern provider. Explicit extern
aliases retain the actual recorded provider and exported macro owner. Absolute paths
start at the defining crate root. Plain use imports also start at the crate root;
import-specific routes preserve the separate relative invocation behavior and check
visibility from the original declaration scope. Later edition routes are preserved.

Eight registered controls cover missing explicit import bindings, explicit alias callbacks,
absolute and nested plain imports, older entry refusal, local invocation, and actual
mixed-edition macOS native comparisons. Native preparation compiles isolated oracle
copies rather than any LRR-selected live runtime unit. Four positive VM/native
comparisons and an E0432 refusal actually pass in this checkpoint.

Complete older-edition includes, generated-token edition compatibility, procedural/
native facets, source origin and invalidation, quota boundaries, runtime/type
integration, full MX027/MX028/JIT, M014–M016 and release acceptance remain pending.
Linux is a later port; Docker is supplementary. Component evidence is not full M013
or release acceptance.

The initial native refusal assumption for `helper::seed!()` was disproved by actual
Rust 1.99.0 compilation. Its failed log is retained; the implementation and separate
positive invocation regression now preserve that behavior. The use-declaration
refusal is a separate program. This is a corrected implementation assumption, not
a specification contradiction.
