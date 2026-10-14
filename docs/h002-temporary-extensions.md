# H002 syntactic temporary-extension annotations — in progress

A bounded pure annotation pass now runs after structural body indexing. It visits
only indexed let statements and expression identities, preserves opaque bodies,
and returns owned ExtendedTo targets with explicit constructor guards. Annotations
and guards participate in the body's stable encoding and metadata budget. No
name/type resolution, expression evaluation, expansion or build execution occurs.

The initializer frontier propagates through parentheses, tuple/non-repeat array
constructors, braced fields, cast operands, block tails, if branch tails and match
arm expressions. An ordinary borrow on that frontier extends its operand's
produced temporary to the containing block. Base propagation follows nested
borrow/deref, field/tuple-index, index bases and parentheses; it excludes index
operands, arithmetic, method receivers and opaque closures. Extending ref patterns
and direct aggregate/or subpatterns extend the initializer's temporary; explicit
reference-pattern wrappers stop that pattern rule.

Call argument frontiers carry every enclosing call's exact structural NodeId as
a constructor guard. Tuple-struct/variant identity cannot be inferred from spelling
or capitalization. Frame storage applies only annotations with no constructor
guards. Guarded candidates retain their default scope until later constructor
resolution proves the relevant identities; that integration remains pending.
Annotations change value-temporary ownership scopes without changing lexical
binding or place ancestry. They do not extend a returned reference's referent by
following its value.

Six syntax controls cover frontiers, exclusions, pattern wrappers, base/index
propagation, exact nested constructor guards and deterministic budget refusal.
Two explicit kernel storage controls initialize owned strings via GuestStore and
Frame: unconditional extension survives inner tail/block cleanup and retires at
the containing block, while an unproven call guard leaves the object in its default
tail scope and produces the existing stale-object outcome there. They do not
execute source borrows/calls or establish managed-reference or user Drop behavior.
The earlier native E2404 baseline remains separate.

Constructor admission, complete default temporary-boundary integration, general
patterns, source owning values and references, owning iteration lifetimes, cleanup
suspension and full E24 off/auto/force destructor/effect/native acceptance remain
pending. H002 remains in progress; no full LRR fixture or release is marked passed.
