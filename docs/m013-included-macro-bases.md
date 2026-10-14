# M013 invocation-file bases for macro-produced includes

M013 remains in progress. Each admitted original expansion records its actual
invocation span as an immutable directory-base fact. Declarative child expansions
inherit that base. An adopted syntax include establishes its captured virtual file
as the base for calls inside that source. The same bounded record identities and
checked retained metadata own these facts; there is no filesystem lookup in the
frontend or query provider.

This distinguishes token provenance from the containing invocation-file context
required by Section49.5. Argument tokens retain their original source identities,
content, editions, origins and hygiene, including substituted tokens from another
file. Generated compiler preparation supplies the retained directory base separately.
Its resolver validates the exact owned invocation token span and base location;
captured included bases must also match the retained view's content, origin, caller
context and authority-bound location. Original-source callers retain their prior
strict physical-source checks. Include output is still reproduced from exact adopted
capture/expansion/path producers before discovery admission and sealing.

The native macOS witness shows that a macro defined by an included file, then called
in main.rs, emits include! relative to main.rs. When called inside another included
file, it uses that file instead. The initial definition-file assumption was wrong;
the witness and failing regressions are retained. This is an implementation fix to
invocation-file provenance, with no change to the selected language/backend/profile.
Capture-origin and token-origin identities remain distinct and are checked against
their respective retained records rather than treated as interchangeable.

Nine new controls cover literal/concat emission from included macros, invocation in
the same or another included file, mixed-source substituted path tokens, source
symlink escape, ordinary/forged output refusal, full compiler-input seal membership
and changed-environment revalidation. Five actual CLI values compare with isolated
native macOS ARM64 Rust1.99.0 programs having conflicting candidate files. The
minimal native probe preserves the wrong-assumption witness independently.

General cross-unit/world generated-token paths and edition interactions, multiple
source/build-output roots, user/procedural string operands, compiler-expression
runtime lowering, full Cargo/environment/facet preparation, remaining module/origin/
quota/runtime/type integration, M014–M016, full MX027/MX028/JIT and release acceptance
remain mandatory. Linux remains a later port. This checkpoint is component evidence;
M013, full fixtures and release are not marked complete.
