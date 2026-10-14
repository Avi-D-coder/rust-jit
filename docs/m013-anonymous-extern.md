# M013 anonymous extern declarations

M013 remains in progress. Captured extern declaration and route names are explicitly
optional in their stable encodings. The pinned AST recognizes an underscore rename
as nameless: it contributes no type candidate or external-module placeholder. Its
source declaration identity, recorded dependency, visibility and macro-use recipe
remain owned and checked. Repeated declarations retain distinct source identities.

Named alias indexes and ordinary module-prefix publication omit nameless bindings.
Anonymous macro-use still registers its crate-wide all/selective prelude and retains
the actual dependency definition owner, including in generated output. Anonymous
preparation still requires a recorded provider; it grants no native runtime symbol
loading or macro execution authority. Dependency module-prefix aliases are no longer
installed as implicit bindings where the source disables the implicit prelude, so
an underscore declaration cannot accidentally expose the dependency's name.

Eight new controls cover original/generated/selective macro-use, local-module
coexistence, repeated original/generated declarations, exact header/type/placeholder
absence and uncaptured-provider refusal. The existing anonymous refusal control now
checks actual incomplete lookup under no-implicit-prelude rather than rejecting the
syntax as unimplemented. Five actual VM values match isolated native macOS ARM64
Rust1.99.0 programs, and an E0433 native witness confirms disabled name lookup.
Explicit oracle preparation only compiles the isolated native copies; no selected
live runtime unit is compiled by LRR and no arbitrary native symbol is loaded.

Rust2015 explicit/prelude rules, complete procedural/native facets, origin overlap/
invalidation and remaining generated module capture, full quota/failure boundaries,
runtime/type integration, complete MX027/MX028/JIT, M014–M016 and release acceptance
remain mandatory. Linux remains a later port. This checkpoint is component evidence;
no full fixture, complete M013 or release claim is made.
