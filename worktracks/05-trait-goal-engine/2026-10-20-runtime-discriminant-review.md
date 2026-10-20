# Runtime discriminant continuation — separate source review

Candidate V480 source fd3acad9cfea34694fe9e2b433cdffba8d9e90b23fd4063f701dfe4890cf672d;
baseline 7233790. This review is source evidence only until actual terminals.

1. No assertion/expression evaluator added. `run_facade` retains its ordinary
   source-selected closed intrinsic and forwards the shared store scalar.
2. No literal-only macro executor or synthetic grammar added. SchemaNames
   binding remains the ordinary source/generated declaration join.
3. `BoundNominalVariant::discriminant_type` is the captured repr integer (or
   Rust's default isize), not an assertion or expected-result guess. The source
   identity also retains schema/definition; stable encoding advances to v2.
   The full actual upstream solver source/root/projection join remains open.
4. No generated-source exclusion. Existing checked binding and constructor
   entry points apply identically. Required full origin matrix is not inferred
   from this source review; runtime tests must establish their own scope.
5. No pattern engine, branch preparation, static/borrow check or new cap.
6. `enum_discriminant_parts` reads the retained TypeDescriptor and existing
   discriminant table; there is no parser, source query or planner in execution.
   Numeric preflight uses `enum_cast_type` without materializing a wide value;
   only execution uses the existing immutable numeric arena. Repr metadata is
   prepared cold during the existing variant binding, without a layout demand.
7. No namespace scan or unrelated invalidation. Currentness follows existing
   source-bound variant plans. Repeated descriptor integer attachment agrees.
8. No body/parser clone or whole-file search. One small source integer and
   descriptor identity are retained; table sharing and frame structures remain.

Remaining routes: `mem::Discriminant<T>` still declares an isize field rather
than the actual associated type; it needs the shared projection/field source
integration. This candidate must not be called complete while that remains.
The intrinsic now refuses an unevaluated expression table entry instead of
returning its ordinal. The existing E013 initializer demand currently handles
casts, not intrinsic calls. Its scalar extraction now uses shared integer_bits,
including i128/u128. Enum images/provenance and full arbitrary field view
support remain open. The stale-cell check in enum_cast_type needs explicit
review against enum_cast_operand before checkpointing.

V479 library compilation failed on one host privacy error and immutable
numeric preflight use of the new allocating scalar reader. Both are repaired
in V480 by a separate nonallocating type read over the same shared parts.
The V479 attempted after-runtime manifest was inapplicable without a successful
after-build record and failed; no V479 build/test acceptance is claimed.

V481 production review restores the same stale-cell check before pointer
inspection in enum_cast_type and reuses the already selected descriptor in
enum_discriminant_parts. V481 source is ddf62b2f8c496fed2a733667648aed35bfbbc21951ceeef834f297a89fcf5ff6.
Both builds pass, unchanged discriminant_size and discriminant_value pass with
complete owning-2015 captures (4684 and4537 matched VM operations). The six
supplemental root-edition runs pass2018/2021/2024 on the byte-identical CLI in
V482; they do not replace upstream's owning edition or count as six fixtures.
V480 repr/wrapper/standard-macro cast controls pass; the mem::Discriminant
combined control fails PathAdjustment and remains failed, without a measured
baseline attribution. V480 initial60-second observation times out; the
same-binary longer observation passes and does not erase the timeout.

V480-V482's attempted standalone metadata control used an incomplete source
graph, then graph-only/current binders without its missing compilation inputs.
Those controls fail, are not waived, and are superseded by extending the
existing complete configured source catalog matrix in preentry_loader_tests.
This removes the unsuccessful extra control/helper rather than weakening
production admission or fabricating source facts. The existing matrix retains
ordinary/user-emitted/wrapper source, source withdrawal, memo and cold-body
assertions, and additionally checks actual bound variants, descriptor integer
identity and full-width scalar bits. These are component observations, not
additional upstream runtime contracts. V483 fails only the host Debug bound
on a generic assertion helper; V484 uses explicit typed poll handling.
The final review fingerprint/evidence and unexecuted matrix remain attached
to the final validation record; no task, architecture or JIT gate is closed.

Final separate source review: V484 fingerprint
dc7b8ea9f4b387b51f6e41692a5c69ff456867f75d189268e151f4dfda9512d8.
The production changes after V480 are only the restored stale check and
eliminating the second descriptor lookup. The eight conclusions above apply
to this inspected source; V481-to-V484 differences are only the two test
files (the unsuccessful extra test/helper was removed). The current CLI is
byte-identical to V481, with sha256
606b46982e61ab1bc5a0fd8b5bffded5eea33c98328b876d0aa3fa8baab7057a.
V484 lib build and both selected controls terminate0; closing source/HEAD/
test/CLI guards terminate0. The complete source harness verifies60worlds,
180enum records,456bound fieldless variants and396literal scalar results,
with48external repr-header variant/scalar controls included in those totals.
It retains memo identity, source withdrawal and no executable body queries.
Full-width Signed/Unsigned Staircase values come from unchanged repr128.rs;
its whole main still fails generic operator Type selection and is not passed
by this component evidence. Actual source/generated/wrapper metadata joins
are exercised, and V480 ordinary/macro/wrapper/standard-macro cast controls
remain attached to V480. There is no blanket final-source suite/milestone
claim. The one measured failing-parent runtime gain is discriminant_size;
discriminant_value has a current pass without a measured failing parent.
All remaining routes and gaps named above remain repair obligations.
