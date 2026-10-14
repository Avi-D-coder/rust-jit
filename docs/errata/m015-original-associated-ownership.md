# E-M015-012: associated producers retain declaration ownership

M015/revision 4 sections 49.3/50.2, integrating the retained M014 declaration
worklist: associated annotations and impl/trait item macro calls are transformed
in their captured declaration slot. Module-item header publication has a separate
owner invariant. Keeping an associated member in the source outline does not
authorize publishing its replacement as a module item or promoting its names.

The official associated-function component with an identity decorator before
its compiler Test annotation selected an ordinary ItemOutline producer. The
real Attribute request used Items, executed one authorized callback, then failed
at generated header parent scope/context validation. The strict compiler Test
refusal and post-fulfillment record/catalog assertions were not reached. The
original ordinary and cold producer identities/scopes are distinct facts in the
retained witness. No Test admission success or namespace defect was inferred.

An LRR-specific ownership regression, installed before this correction, retains
the original method DefKey and impl owner, a root free-function annotation and
an associated macro call. The strict module/declaration ownership assertions
failed because the associated annotation entered the ordinary worklist.

The existing Outline now emits ordinary attribute producers, blockers and
textual frontiers only for items without an associated owner. Associated item
macro calls stay out of its ordinary invocation/frontier list. The declaration
scan still captures their exact AST slots, lexical scopes, original ranges and
producer identities; SourceDiscovery already schedules that separate inventory.
Associated ItemOutline entries, header recipes, method DefKeys and impl owners
remain retained. Free-function annotation routing is preserved. No captured
scope/member key is guessed or manufactured, and module header admission is
not weakened to allow an associated owner.

The focused ownership regression passed after the correction. Private joint
validation passed library194/adapter30/scan67/source61, unchanged associated and
local Attribute controls, formatting and strict Clippy; broader validation passed
65 controls across13 targets. The independent unchanged-source cold decorator
control passed one actual callback and record in ImplItems, retained the compiler
Test marker, and preserved the strict free-function diagnostic before sealing
and each tested catalog/filter request. Parent independently passed both source
and native controls with no body query. See the retained receipts
artifacts/macos-first/m015-required-admission-repair-original-associated-ownership-catalog-joint-v1-acceptance.json and
artifacts/macos-first/m015-required-admission-repair-original-associated-ownership-cold-native-joint-v1-acceptance.json.
Main integration and fresh milestone remain pending.
M015/full MX031/MX032, stock fixture parity, JIT, release and Linux remain pending.

Integrated combined macOS milestone passed 2042 actual host/doc tests/3517 fixed source/include inputs and membership/all9checks0. Receipt artifacts/macos-first/m015-associated-binding-complete-main-acceptance.json; full task, stock, JIT, release and Linux acceptance remain incomplete.
