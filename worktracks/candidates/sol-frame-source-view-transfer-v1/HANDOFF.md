# WT04-02 / LOCAL-010: retained frame source transfer

Status: source-only candidate, uncompiled and unexecuted. No canonical source, target, Cargo, native/JIT or ledger changes. This does not complete M018, M017, T032 or TR003–TR005 and makes no runtime-contract/full-fixture claim.

## Invariant and scope

Revision 5 §52.2 requires `Parameter declared T → Param(T)` with the actual descriptor kept separately, and `let y = x → preserve x’s source view`. §52.1 explicitly forbids overloading `ValueRepr.type_id` as source identity. T032 checks incompatible actual parameter/return contracts at their reached boundaries.

This slice hardens storage transfer of already-retained legacy `TyTerm` fields in `Frame`. It does not attach the earlier frontend `SourceViewPlan`, mint a `SourceViewId`, prove current namespace authority, select members or force bounds/dictionaries. Equal runtime descriptors cannot substitute one Param/nominal identity for another. Missing source metadata remains absent. No future body/thunk/query or numeric default is introduced.

Three exact candidate files:

- `crates/lrr/src/vm/frame.rs`: module/test attachment, conservative transaction metadata reserve, narrow parameter/local/bind/result/return preflight hooks; scalar parameter/local reads use the private transaction.
- `crates/lrr/src/vm/frame_source_transfer.rs`: private single-use transfer witness and checks.
- `crates/lrr/src/vm/frame_source_transfer_tests.rs`: 11 acceptance controls outside originals.

`production.patch` includes the cfg(test) attachment; apply production and acceptance together (`complete.patch`) for validation. The base contains only the exact original frame file. The patch is bounded; do not overwrite another contributor’s whole frame file.

## Private transaction and currentness

`RetainedSource::{Parameter, Local, Slot}` selects an existing frame storage endpoint. `prepare_retained_source_transfer` first calls immutable preflight and then owns a private non-Clone snapshot. No public constructor exists. The snapshot binds frame anchor, exact PC and immutable installed signature; source endpoint/cell/epoch/value/term; destination cell/epoch/source/hint/strictness; and expected exact output value/descriptor.

`require_retained_source_transfer` rechecks the snapshots and immutable admission before allocation/move. `attach_retained_source_after_write` consumes the witness after Frame::write, checking active destination scope, actual cell owner/epoch, cleared metadata, exact output ValueRepr and descriptor. Failed post-write attachment retains the actual value in owned destination storage with absent metadata; it never publishes an unowned ticket. It preserves the incoming term, including its original reference region, rather than substituting an expected term.

Primitive/empty nominal/primitive-array/reference views are checked against actual descriptors using existing storage/borrow shape checks. An owning Param must belong to this frame definition; if GenericEnv exists, the exact typed ordinal must be declared and any runtime substitution must match the actual descriptor. Context-free legacy Params retain only storage identity; this is not new namespace/typing authority. Existing GenericEnv cannot be replaced after frame initialization.

The witness is synchronous within the owning run’s Frame/GuestStore action. It must not be serialized, cached across NeedInput, or used to bridge unrelated GuestStores merely because numeric IDs coincide. Existing run/input revalidation remains the root engine’s prerequisite. The exact M017 current discovery seal/catalog/namespace witness is still missing for general frontend plans; that interface cannot be supplied by a lexical-call-specific certificate.

## Fuel, ownership and metadata

Before read opcode fuel, all source/descriptor/strict-destination checks and ticket/cell capacity predictions run without guest mutation or ticket publication. Only strict storage destinations compare expected source identity; forwarded/non-strict context and the empty root result retain the reached actual term. Distinct declared/actual Param results fault at the reached return preflight, not a cold body or earlier forwarded read.

Frame metadata admission reserves one transaction owner and both bounded source trees. Closed legacy tree measurement uses fixed arithmetic, avoiding the temporary traversal Vec in TyTerm::heap_bytes. Unsupported old destination trees refuse before comparison/cloning. Bounds and total frame metadata use checked arithmetic and the existing 32 MiB ceiling. Source trees are cloned only after admission. Existing source-term allocation/clone conventions remain unchanged; this adds no general source table or allocator framework.

Parameter/local scalar writes consume normal read tickets and Frame::write, then attach. BindLocal preflight checks actual source ownership/descriptor/heap and combined cell/ticket quotas before its existing move/attachment. ResultTransfer receives additional immutable checks through Frame::preflight_transfer; vm/step.rs is untouched, so its existing certified-root-source transfer remains the executed route. Replacing that route with the single-use witness must preserve normalized deferred alias certificates; do not attach raw source over a separately certified root result. Return gains owned source checks and a distinct-Param check at its reached boundary.

Aggregate Copy activations are not wired to this scalar witness (an aggregate copy changes object identity). Existing aggregate activation and source attachment remain separate. Mutable/immutable reference lease operations remain existing GuestStore checks. General abstract/composite/member/field/reborrow/default-Self propagation is still a prerequisite, not covered success.

## Original fixtures and controls

Upstream `rustc/rust` commit: ae0fe511b15ab760d7e963b13389d1bc2ab42b18.

- `tests/ui/traits/copy-trait-implicit-copy.rs`: unchanged run-pass file; explicitly select its `can_copy_copy<T: Copy>` body for a genuine lowered parameter read and first let commit, then a direct legacy storage read. Main is empty; this is not a full fixture/main exercise.
- `tests/ui/generics/generic-fn-infer.rs`: unchanged run-pass file; explicitly select its `id<T>` body for a genuine lowered parameter read and reached Return recipe. This does not execute the fixture caller/assertion or establish caller substitution/resume.
- Both component controls cover 2015/2018/2021/2024. Fixtures require no auxiliary crates, revisions or expected output companions. The preserved original license texts and fixture headers are unchanged.

Generic-fn-infer is already in MAIN’s upstream manifest. Copy-trait-implicit-copy is not; root must import it unchanged through the official importer before compiling this acceptance child. Original hashes/bytes and license hashes are in preregistration.json.

Nine LRR controls supplement the two original components: distinct rigid strict destination with equal I32 descriptor; non-strict forwarding and genuinely declared T/U mismatch at Return; exact fuel/replay prefix; cell/ticket and transient-metadata refusal; stale cell epoch and wrong output payload; changed PC/frame/installed context; absent local source; distinct nominal strict context; and corrupted actual parameter descriptor under a genuine argument seal. All are outside the upstream directory. The wrong-return program is a runtime-boundary control, not a rustc compile-pass substitute.

## Actual checks and pending validation

Rustfmt 1.99 with `skip_children=true` is required for this intentionally bounded overlay; ordinary module traversal initially refused because untouched frame child modules are not copied into the overlay. The final formatting check and `git apply --check` against both exact base and current MAIN were collected separately. These are source checks, not compile/test passes.

Root validation proposal: merge this bounded patch into a fresh frozen source candidate with the current decoder/alias work and imported Copy fixture, then run the new frame child, existing T032 reached-return controls, existing computed-reference/aggregate/metadata/call-packet banks and the mandated integrated milestone. Collect actual async terminals before integration/claims. No worker builds or targets were created.
