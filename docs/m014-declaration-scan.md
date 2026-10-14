# M014 declaration skeleton and impl-header inventory

M014 is in progress. The invariant for this integration is to discover active
nested impl headers and macro producer demands without lowering executable bodies,
resolving types or executing macros. This is a structural inventory, not a completed
ImplCatalogSeal or a reached method-selection result.

The scanner uses the pinned rust-analyzer tree. It traverses function, block,
closure, async and const-expression syntax; applies existing effective cfg facts;
and prunes disabled subtrees. Macro calls and transforming annotations remain
explicit producer records. Macro token trees stay opaque until a later exact
identity/grammar-slot decision; `stringify!` is not guessed inert by spelling.

Impl and associated-item headers retain owned token recipes, exact parse/edition/
cfg/source-path identities, module/function scope and enclosing container ranges.
Method bodies are excluded from header recipes, including methods embedded in
signature const expressions. The header token walker skips body subtrees, and
function-owner header identities are memoized inside one scan. Actual structural
node/header-token costs are reported separately; no executable-preparation counter
is fabricated. Retained metadata uses conservative checked accounting.

The immutable index sorts source inventories and entries by stable identity.
Repeated headers in different captured files remain distinct; submitting the same
inventory twice is refused. Unknown producers stay pending. Syntax errors, node/
depth/count/metadata limits and source conflicts return typed failures rather than
partial or empty successful inventories. Neither the scanner nor index can issue
a catalog completion certificate.

Nine targeted macOS controls pass. Two consume unchanged official rustc source:
`local.rs` contains ten impls inside functions/modules/anonymous consts, while
`weird-exprs.rs` contains six inside signature, array and discriminant expressions.
Their original 2021 editions, directives and lint expectations remain unchanged.
The tests parse declaration-only facets in that recorded edition; live entry still
requires Rust 2024. These are syntax/inventory component assertions, not native
lint equivalence or full LRR fixture acceptance.

Additional controls cover disabled subtrees, cold unresolved ordinary types,
closure/async declarations, opaque macro arguments, physical-file identity,
deterministic input ordering, lexical context, duplicate refusal, syntax failure,
header/body invalidation and checked inventory limits. The initial API-missing
failure and corrected strict-Clippy failure are retained. Gate00/01/02 and provisional Gate03 component pass with 1693 host/doc tests,
all 731 M013 controls and all nine M014 controls. Formatting, strict Clippy for
runtime/external host tool, architecture and first/final manifest audits pass. The
restricted unchanged empty-derive interpreter/native reference was rerun. Fifteen
licensed cases and all 35 imported file hashes retain 38 component assertions;
full compiletest/LRR acceptance and guest JIT acceptance remain zero.

Next M014 work must resolve local macro namespaces, apply audited intrinsic
grammar-slot summaries, admit expression/block expansions, scan their actual
declarations and complete unit/facade producer coverage. Those facts must integrate
with catalog sealing and later symbolic impl/dictionary selection. MX029/MX030,
guest JIT, full fixture, release and Linux acceptance remain pending.

See [the checkpoint](../artifacts/macos-first/m014-declaration-scan-checkpoint.json).
