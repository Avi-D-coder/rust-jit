# T040/M018 nominal memory images — separate source review

Baseline654e0c79cda8975972f1f3aedb5ab822182e0b3b. V445 candidate fingerprint
e0a99795da60ef150a4459ec360f38445ccdadb7f6e06cdebaa5efb068bf6deb.
Inspected GuestStore::{plain_type,nominal_image_fields,write_plain_image,
read_plain_image,transmute_value}, DeclarationFacts/LayoutTable instances,
RunMachine::ensure_layout_facts/declaration_layout_facts and run_facade::execute
Transmute case, begin_construction/construction_push/commit_construction,
aggregate_cleanup_step -> aggregate_cleanup_transfer_inner(false), discard.
MA002/MA003/MA006 are separately numbered legacy obligations and stay held.

1. No expression/assertion parser/compiler/evaluator. Mem transmute/zeroed are
explicit existing runtime/library intrinsics, selected by the same actual source
member service; only their common storage kernel is extended.
2. No literal/union-only executor, opaque phases or verifier route. Ordinary
VM-backed const initializer calls the same existing facade intrinsic and creates
real ordinary aggregate field cells through construction guards. Union values
are still unsupported; this is only a required common representation component.
3. No macro-selected numeric constraints or method whitelist. Field types come
from actual prepared nominal instances, offsets from actual repr and the one
runtime/layout::univariant algorithm. Image representability grants no Copy,
Clone, Drop selection or source name identity.
4. Original and generated nominal declarations join at actual TypeSchema,
SchemaNames and engine declaration_layout_facts. The image helper only sees the
resulting run-local descriptor/declaration/instance facts. No original-only
exclusion or generated-authority bypass is added. All4edition/origin runtime
matrix is required and currently unexecuted on this fingerprint.
5. No new patterns, static checking or eager executable body preparation.
Declaration field metadata needed for an actual image is the reached layout
operation's required dependency; no cold initializer/branch/callee is lowered.
6. No source parser/query provider/AST/source plan is entered from installed
image execution. Geometry is computed by the existing runtime layout kernel;
nominal field descriptor/offset lists are O(field count) and physical image work
is O(bytes/fields), like existing tuple/array images. No performance claim or
claim that these runtime geometry lists are persistently cached. Source layout
facts remain retained and actual generic fields are instantiated by layout_demand.
7. No namespace-map construction or census and no source dispatch guard change.
Runtime byte traversal follows only the reached value/type's fields.
8. No BodyIndex/parser clone or repeated whole-file node search. The field
vectors copied here are small runtime descriptor lists, not immutable bodies.
No new limits/accounting/fuel, sandbox, unsafe or trait-solving algorithm.

Removed alternate/refusal routes: blanket nominal exclusion in common byte
classification/encoder/decoder, obsolete negative tests requiring nominal images
to be refused, and orphaned Transmute source tickets skipped via moved_in.
The new signature accepts the real source ticket. On success the same owning
field-transfer kernel consumes it and descendants, without scheduling guest
Drop; result reconstruction uses ordinary guards/ownership. It does not use
mem::forget to retain a graph for the run. Fatal image failures remain fatal
teardown and cannot be counted as cleanup or runtime acceptance.

Review limits: unions/enums/tag/niche image reconstruction, provenance-bearing
images, union coherent byte backing/views/references/moves/Copy and cleanup are
still missing. Existing image window helpers return zeros/ignore writes on an
out-of-range window; callers currently supply layout-sized images, but those
helpers must become explicit checked failures before shared backing integrates.
This is a named bounds-hardening obligation, not authority to expose them as a
memory API. Source review identifies it; no new out-of-range behavior is claimed.
Destination Drop/source no-Drop, actual const VM initialization and scalar/image
regressions require actual terminal evidence. All feature/task/interpreter/JIT
and full architecture gates remain held.

V445 actual terminals: both builds0;3/4controls pass. The full16edition/origin
const-image control, source-no-Drop/destination-once control and actual size
mismatch control pass. The retained combined scalar/pointer/array control fails
in formatter source signature preparation (`&mutFormatter<'_>`), not an accepted
regression. Check that exact unchanged control against654e0c7 production in the
direct checkout: temporarily restore only cell_layout.rs/run_facade.rs to HEAD,
retain the exact candidate patch for restoration, build/fingerprint V446 and run
only that control. Do not change HEAD or any test source during this controlled
baseline. Its output and closing guards must be collected before restoring the
candidate; V445 runtime evidence remains bound to V445, not the baseline.

Maintained V444 baseline owning2015 transmute-const faulted at19dispatches,
3calls/1real initializer. V445 newly completes58dispatches,3calls/1initializer,
unit/empty streams/cleanup. Additional unchanged transmute-array-to-scalar fails
at6dispatches/0calls on explicit Type source-signature admission, retained as a
failure; no overall batch pass or broader array regression inferred.

V446 controlled baseline terminal: lib build0, exact retained control101, source/
HEAD/binary closing guards match. Its failure line equals V445 exactly:
`2015 LRR-E-UNSUPPORTED: call contracts require later source type integration:
type &mutFormatter<'_>` (rendered with backticks in the actual log). Only the two
production files were restored to HEAD654e0c7; all other production files were
already unchanged from HEAD, and the tested existing control is unchanged.
After terminal/guard collection the saved candidate patch restored V445's exact
source fingerprint. This establishes the formatter failure as inherited, not a
new image regression; it remains an unmet test and is never marked passed.

V447 candidate fingerprint2c16433a7df879f885bea802c04de7714a4c2d2bad88eb7ceb07ed6e3689ff20.
Window helpers now check offset conversion, end addition and image ranges;
scalar reads can no longer invent zeros on a missing window and writes can no
longer silently ignore it. Full field layout window validation precedes image
recursion/guard allocation, including zero-sized end windows. This closes the
specific silent-window obligation above, subject to actual boundary tests;
relative field offsets still come exclusively from the one admitted layout
algorithm. No quota/cap or memory-accounting check is added. The ownership
control also exercises zeroed nominal destination Drop. Exact source-ticket and
object/guard retirement checks are runtime ownership controls, not a new Rust
language suite or permission to replace unchanged upstream witnesses. The
V44516origin and source/runtime passes stay attached to V445; later changed
windows/ownership have new scoped validation, not a relabeling of those receipts.

Final exact review/evidence at V447 fingerprint
2c16433a7df879f885bea802c04de7714a4c2d2bad88eb7ceb07ed6e3689ff20:
both builds terminal0;3/3scoped controls terminal0; maintained owning2015
transmute-const terminal0 and runtime-contract pass,58dispatches/3calls/1real
initializer, complete unit/empty streams/cleanup. All closing source/HEAD/binary
guards match. Boundary control proves original ticket and aggregate object are
consumed, no leftover guard/owning object/ticket beyond the result, source field
locations retired; missing/overflowing windows neither mutate bytes nor create
owners, and zero-sized end windows remain valid. The4edition destination control
now exercises both transmute and nominal zeroed, source no-Drop and exact-once
destination destruction. The earlier16origin control remains V445 evidence.
The identical inherited formatter failure and explicit-Type array source-signature
failure remain failed entries. One newly passing stock case against measured
pre-change source, no suite total/card/milestone/full-feature or JIT closure.
All8source conclusions above cover this exact source. No source edits follow
this review before checkpointing. Union storage/alias views/provenance/cleanup
remain incomplete and cannot be substituted by this image component.
