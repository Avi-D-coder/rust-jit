# M013 original helper inheritance through attributes

Task: M013, revision 4 Section 50 discovery and Section 57 ordering. The invariant
is that helpers introduced by completed original-source derives follow replacing
attribute output, retain hygienic item-local identity, and stop at function-like
or derive expansion children. They never become global helper names.

The before log records an actual gap: the derive and identity attribute execute,
then discovery stays incomplete on the retained helper. The acceptance adaptation
starts with the second item of the unchanged rustc `attribute-after-derive.rs`.
Outside the licensed import, it substitutes SDK checking exports and helpers,
uses a tuple constructor to exercise value lookup, and retains an enabled helper
member plus a disabled cfg member. The copied source, auxiliary, stdout, license
texts and import hashes remain unchanged. This is component coverage, not a full
stock-fixture execution or exact diagnostic/stdout compatibility claim.

The original derive queue retains a bounded helper environment only after replay
of every preceding completed occurrence. Lookahead metadata may support member
classification but does not introduce helpers from a later group. The environment
uses the pinned adapter's item-name context classes; there is no alternate hygiene
algorithm. Its stable fingerprint binds the original producer, completion prefix
and sorted helper/context bindings. The attribute selection receipt and final seal
replay include that environment. Retained bytes and binding counts are checked.

The source attribute's actual output catalog receives this environment only for
the exact original producer and attribute expansion kind. Existing attribute-child
inheritance carries it to subsequent attributes. Function-like and derive children
do not inherit it. Matching still requires a bare name and the exact hygienic
class, and publication still requires the owning MacroRun and freshness checks.
No build preparation or macro execution is added to a query provider.

Four targeted macOS controls pass: one derive and one identity attribute publish
the constructor and earlier constant; two forwarding attributes retain the same
environment without rerunning the derive; a function-like child emitting the
helper remains incomplete and cannot publish; and an unrelated original sibling
helper remains unresolved. The negative controls are refusal/boundary evidence,
not passed language fixtures. Gate00/01/02 and provisional Gate03 component pass with 1684 host/doc
tests and all 731 M013 controls. Formatting, strict Clippy for runtime and external
host tool, architecture and first/final manifest audits pass. The restricted unchanged
empty-derive CLI interpreter/native reference was rerun. Thirteen imported cases
retain 36 component assertions and all 32 unchanged file hashes. Full compiletest
and LRR fixture acceptance remain zero.

M013 remains in progress. Complete legacy ordering/lint/helper diagnostics,
module/cold-body/resource/world/edition/service/CLI matrices, full fixtures,
guest JIT, release and Linux acceptance remain pending.

See [the checkpoint](../artifacts/macos-first/m013-original-helper-inheritance-checkpoint.json).
