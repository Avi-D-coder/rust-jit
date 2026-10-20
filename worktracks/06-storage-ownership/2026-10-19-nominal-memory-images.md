# T040 / M018 — ordinary nominal memory-image prerequisite for union storage

Baseline654e0c79cda8975972f1f3aedb5ab822182e0b3b, LOCAL049 solo root.
Read WT06, active claims, E005/E006/E007 and spec12/57. The unchanged runtime
witnesses union-transmute.rs, union-align.rs and align-struct.rs require actual
common storage and nominal members. Existing cell_layout byte images serve
mem::transmute/zeroed but exclude all nominal values; even Align16(u8) cannot
round-trip through that shared representation. Independent union field cells
would preserve a different semantics and are forbidden. No union feature or
fixture closes from this prerequisite alone.

Extend existing GuestStore::{plain_type,write_plain_image,read_plain_image}
through prepared DeclarationFacts/LayoutTable instances and the one
runtime/layout algorithm. Original/generated declaration metadata already joins
at SchemaNames/declaration_layout_facts; no source parsing/planning/query enters
image reads/writes. One source-derived struct field-layout helper supplies both
encoder and decoder: materialized fields, actual repr placement, variant0.
Use existing begin_construction/construction_push/commit_construction and its
ordinary ticket/field ownership. Never synthesize a source constructor or grant
Copy from byte representability. Library shapes with a different storage model,
enums/tag/niche reconstruction and pointers with ownership/provenance remain
explicit missing image infrastructure until actually implemented. Real plain
nominal fields must be supported in the common image path rather than a
union-only codec.

Primary unchanged focused prerequisite witness: consts/transmute-const.rs,
repr(transparent) Foo(u32) through a real reached const initializer and selected
mem::transmute. Baseline capture V444; retain any failures. A runtime origin
control reuses that unchanged item stream in original, arbitrary emit, wrapper
and standard-macro contexts across2015/2018/2021/2024, through ordinary VM routes.
Add LRR-only layout/ownership controls outside imported originals as needed.
A source-aware review covers all8 recurring failure modes, separately from
MA002/MA003/MA006. This is an audited intrinsic/storage primitive, no alternative
Rust evaluator, macro semantics, matching, solver, static checking, budgets or
new unsafe. Layout preparation stays cold and retained; runtime image cost is
O(reached bytes and fields), no full namespace map or deep BodyIndex copy.

Required next integration remains shared union backing bytes with typed views,
field projection/write/reference coherence, initialization, full-value moves/
Copy and union-specific cleanup (no automatic field drop), plus relocation/
provenance support. Direct pointers or ownership-bearing fields cannot be
faked as independently owned struct fields. Keep unchanged whole witnesses
failing until they execute fully; no JIT/card/milestone/full-fixture closure.

V444 confirms the unchanged const witness reaches ordinary transmute and fails
at the nominal byte-image exclusion (no compiler/native execution). Source review
also finds Transmute places its source ticket in moved_in without consuming it.
Replace that orphaned-ticket route with actual physical consumption after image
reconstruction, using existing aggregate_cleanup_step/discard transitions but
without scheduling guest Drop (Rust transmute transfers bits and never drops the
source). A byte image is not Copy/Clone evidence. Test source destructor behavior
and ticket/cleanup ownership outside imported originals. Do not use mem::forget
as a substitute that would leak each input graph. On a fatal image error retain
normal fatal teardown; do not fake guest cleanup or completed output.

V445 maintained owning2015 runtime: unchanged transmute-const newly completes;
transmute-array-to-scalar remains blocked before the image kernel at explicit
Type call source-signature admission. Both stay classified by actual runtime
contract, not process exit alone. The origin/ownership control batch is still
running; no full milestone or union runtime acceptance is inferred.

Before using image windows for backing storage, replace silent out-of-range
put_bytes/read take fallback with checked conversion/addition/range errors.
No new cap: all in-bounds windows, including zero-size at allocation end, remain
valid. Add a focused storage-boundary/ownership control (not a new language
suite) proving source tickets/aggregate owners are consumed and insufficient
windows cannot fabricate zero bytes or ignore writes. Reuse final unchanged
stock capture/origin evidence at its original fingerprint; later changed source
needs explicitly scoped checks and a new final fingerprint, not relabeling.
