# M020 / LOCAL-049: demanded candidate indexes at publication

Bounded precode reservation: `traits/upstream/prepared.rs` and this record only.
Parent owns `source_factory.rs`, registration, root admission, validation and
checkpoints. No Cargo, Git, fixture edit, worktree or copied target belongs here.
The unchanged upstream witness remains
`fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/repr128.rs`.

Violated invariant: a genuine complete per-trait source bucket and typed heads
cannot receive the pinned candidate index until coarse `PreparedProvider::ready`
also accepts every unrelated compiler-query family. Sparse actual fact
publication must install the exact available bucket indexes without claiming a
complete source/query closure, admitting a root or filling unavailable buckets.

The shared route to extend is `PreparedProvider::publish` after actual complete
`ImplTraitCatalog::require_translation`, raw/canonical source attachment,
source/generic identity, typed trait-head and sorted candidate coverage checks.
Extract the existing pinned `simplify_type` / `TreatParams::InstantiateWithInfer`
classification and complete-index comparison from `ready` into one cold builder.
Remove that lazy construction route from `ready`, retaining all of its existing
coarse snapshot checks. `prepared_impls` must still distinguish absent/unprepared
traits from genuine published empty buckets. Flags never certify coverage.

Pinned `rustc_type_ir/src/fast_reject.rs::simplify_type` reads only the outer Type
shape except its dyn principal `trait_is_auto` callback (line 115). Before facts
freeze, supply that exact callback with a separate `ScopedCache` containing only
actual needed principal trait IDs and Known automatic flags from these same
unpublished facts. This is an immutable cold preparation scope, like Q89's
separate callback scope, not a facts-ready/root certificate. Missing required
metadata is a preparation failure; it must not become a blanket entry or false.
Keep provider/context identity and destroy scoped inputs before installation.

The common original/generated source identity and runtime route remain unchanged.
No candidate matching, custom solver, whole-body/static checking, evaluator,
resource cap, generated exclusion or native guest compilation is introduced.
Cold classification scales with the available complete candidate buckets and
their actual heads; installed lookup retains its existing indexed operation.
No namespace census, source parse or BodyIndex deep copy is added to dispatch.
`DefinitionReceipt` can derive Clone only because it contains immutable Arc
source attachments, identity and scalar role metadata; cloning adds no currentness
or root permission and performs no syntax search or tree copy.

Acceptance required from parent: compile/build on pinned macOS Rust 1.99.0;
validate partial neighboring domains with complete/empty/unprepared candidate
buckets, exact typed-head/raw-canonical/source coverage, dyn principal required
Known flags and context scope, repeated publication and coarse-ready holds.
Then actual demanded root/inference/caller/response and unchanged runtime witness
remain separate. Until those terminals exist, no runtime, full fixture, milestone,
interpreter, JIT or performance gate closes.

## Source handoff and separate source review

`prepared.rs` candidate SHA256:
`e48c76ec05e4b948b2e9cff697ca530da2177fe2b0b70ac0ba0460c87331eac5`.

API: `DefinitionReceipt: Clone` now shares its immutable source/header/context,
parser and generic-declaration Arc handles. Existing Q75 `receipt().clone()`
can therefore supply the exact receipt to the parent's sparse fact factory.
No other new public API is introduced. `CandidateIndex::prepare` is a private
cold builder; `PreparedProvider::publish`, `ready` and `prepared_impls` retain
their signatures. The builder runs after all prior publication validation and
stores the completed index before facts freeze. Early repeated/active publication
fails before entering a temporary callback scope. Sparse trait rows with an
unprepared or inapplicable implementations domain receive no index entry;
Known(empty) receives a genuine empty entry only after the existing actual
complete membership check. Coarse `ready` still requires every prior compiler
query slot and complete associated leaves, then verifies installed indexes.
It grants no new root authority.

`CandidateClassificationInputs` is a separately named exact callback scope,
allocated by `with_provider` alongside Q89's elaboration scope and included in
provider equality/hash identity. Only actual covered heads are inspected to
collect the callback closure: unwrap Pat as pinned simplify_type does, then
read the actual dyn principal and require its Known automatic flag. The pinned
function alone chooses constructor versus blanket and the simplified key.
`trait_is_auto` context-checks this scope and refuses uncertified IDs. Outside
this scope it retains the ordinary published fact query. RAII empties the
ScopedCache before the builder returns, including errors/unwind; no scope value
is installed in facts or retained beyond preparation.

Separate source-review pass, all eight recurring architecture modes:

1. No expression/assertion compiler or evaluator: changed code is retained
   source metadata and pinned outer-Type candidate indexing only.
2. No macro-output execution vocabulary or context-specific parser is added.
3. The exact same `simplify_type` with `TreatParams::InstantiateWithInfer`
   chooses all index keys; no numeric/member selection or independent candidate
   applicability rule is introduced. Callback preflight only collects actual
   upstream reads.
4. Receipt Clone shares the existing original/generated Q75 attachments; it
   neither excludes generated source nor manufactures an identity/currentness
   receipt. The actual publication source checks remain intact.
5. No patterns, bodies, cold branches, static checks or root evaluation run.
   Only Known complete implementations domains seed indexes, independent of
   unrelated bool/opaque/member/constant domain availability.
6. Candidate indexing moved from `ready` into publication once. Installed
   `prepared_impls` still reads immutable indexes; `ready` does no rebuilding,
   parsing or source preparation. No phase-specific plan reconstruction.
7. Existing complete source bucket validation is unchanged; no new namespace
   census or broad invalidation is added to execution. New maps/sets and the
   exact callback collection are cold publication work only.
8. Receipt Clone duplicates Arc handles and scalar identities, never immutable
   parser/body trees. Builder heads retain Copy upstream handles. No whole-file
   search or BodyIndex copy appears in the changed paths.

Cold work is O(T + I log T) plus the existing sorted bucket comparisons and
needed dyn-principal map operations, for published trait rows and typed impl
heads. Lookup costs and installed callback behavior are unchanged. This is
structural review only; there is no measured performance or runtime claim.

Command note: the first edit attempt via `/usr/bin/python3` exited 69 before
running its script because the host reports an unaccepted Xcode license. The
same edit was applied with the bundled Python runtime. No license change or
privileged workaround was attempted. No Cargo, Git, build, test or guest
execution command was run by this worker. Parent owns compilation, independent
review and all actual acceptance evidence; the unchanged repr128 root and all
runtime/interpreter/JIT gates remain held.
