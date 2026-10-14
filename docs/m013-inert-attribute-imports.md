# M013 inert-attribute ambiguity in unqualified macro imports

M013 remains in progress. The pinned adapter now exposes immutable name/identity
metadata from its existing INERT_ATTRIBUTES inventory, copied upstream from rustc.
Identity uses explicit checked lengths and domains binding the frontend commit,
Rust1.99.0 and actual inventory name. Upstream types remain inside the adapter;
no attribute expansion handler, native symbol or user-definition certification is
created by this metadata lookup.

An active unqualified named import that selects a real visible textual user macro
also considers the compiler inert-attribute candidate. If both exist, route creation
reports LRR-E-MACRO-AMBIGUOUS with both identities. Original/generated sources share
the same check. no_implicit_prelude does not remove the compiler inert candidate.
Bare calls retain normal textual shadowing; explicit crate-qualified exported macro
imports bypass this unqualified conflict. cfg-disabled imports contribute nothing.
The active builtin test/derive attribute macros are not members of the inert list
and preserve their distinct shadowable behavior.

Nine new controls exercise seven inert-name conflicts, disabled preludes, bare user
shadowing, imported test/derive names, qualified exports, generated imports, cfg
exclusion and stable/bounded metadata. Nine successful CLI values and nine E0659
refusals agree with actual isolated native macOS ARM64 Rust1.99.0 programs. The
separate 29-case native probe distinguishes builtin-only imports, user imports and
bare calls. The original accepted-LRR/native-E0659 witness remains unchanged as
historical failing evidence; the new resolution record references actual regressions
and current source hashes. Neither historical comparison_passed=false nor a prior
checkpoint is rewritten.

This resolves the recorded user-path inert-attribute collision. It does not establish
complete compiler/SDK attribute-name inventory parity or support all builtin-only
imports and attribute dispatch. Recursive user output macro/string expressions,
additional SDK facets and macro kinds, procedural strings, cross-unit/world/edition
paths, multiple source/build-output roots, full Cargo/environment/facet preparation,
compiler-expression lowering, remaining module/origin/quota/runtime/type integration,
M014–M016, full MX027/MX028/JIT and release acceptance remain mandatory. Linux remains
a later port. Component evidence does not complete M013 or release acceptance.
