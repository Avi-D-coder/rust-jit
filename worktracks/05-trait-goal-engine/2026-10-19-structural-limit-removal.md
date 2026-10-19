# E-R5-006 — remove legacy structural resource guards

Bounded LOCAL-049/LOCAL-024 (same solo owner), WT05-02/M020 prerequisite.
Baseline HEAD c3e9ed88a9aee95cd5200e0b8fd2a95f3268fc9a. This is removal
of invalid interpreter restrictions, not a new solving algorithm or completion
of the upstream solver adapter. M019/M020 and interpreter/JIT gates stay open.

V367 follow-up: deleting the semantic cap exposes host test-thread stack
overflow in the existing recursive transform/closed traversal. Replace their
native recursive traversal with one explicit Visit/Assemble work stack and a
typed binder scope stack, retaining identical child/argument order, simultaneous
substitution, lift/cutoff and placeholder rules. Delete the old recursive
transform helpers, do not add a type-specific deep-reference route or raise
test stack size. This is the same structural substitution, not a new trait
solving algorithm. The original depth300 source control remains unchanged.
The separate existing VM universal-region negative control failed at shallow
source-view admission; production frame code is unchanged in this repair,
baseline attribution unverified. Retain that failure pending its own review.

Separate V369 host-expectation correction: frame_source_types.rs already
treats reference regions as non-data; Frame::supported_source consults that
shared structural service first. The old universal-region test expects all
three metadata setters to reject the source FnType's actual placeholder. That
static restriction conflicts with E-R5-006 and current production behavior.
Correct only the existing test in vm/frame.rs to check retained source views,
unchanged guest-operation/handle/scope state and the real parameter reinstall
guard. No production runtime rule changes, fabricated metadata or weakening
of transaction/binder checks. Preserve V367/V368 failures. Production V368
runtime evidence stays attached to its original fingerprint; rerun only the
affected host control after the final builds. This is not original main
execution or full-fixture acceptance for the regions check-pass source.

Invariant: structural signature substitution must not reject finite valid type
metadata because LRR imposes depth256, projection work100000, or resource
accounting. Those are not the pinned compiler's configured solver recursion
limit. Semantic binder indices, transaction identity, occurs checks, namespace
and placeholder visibility remain unchanged. E-R5-006 overrides old budgets.

Primary source witness: unchanged imported tests/ui/generics/generic-fn.rs,
actual id<T>(x:T)->T signature, all four editions. Supplemental host boundary:
instantiate that real signature with 300 nested shared-reference type terms;
exercise the ordinary SignatureSubstitution, Substitution and Unifier route.
This metadata control is not execution of stock main or a stock runtime gain.
Verify the already imported unchanged method-on-deeply-nested-struct-perf.rs as a runtime probe, with
its actual recursion_limit1024 and conditional blanket impl; do not claim it
necessarily exceeds the legacy256 threshold or passes other missing features.

Shared entries: cli::capture_with/PreparedSnapshot::publish -> SyntaxProvider
FunctionSignature memo -> SignatureSubstitution::{new,constrain_argument,
constrain_result} -> subst::{apply,shift,validate_closed,instantiate_binder}
and unify::Unifier::unify. Original/generated syntax already joins before
signature demand; no new source/parser/execution route. term::{measure,
validate_opaque_shape} currently reintroduces the same cap on nested metadata.

Remove MAX_DEPTH, traversal steps and Budget accounting in these connected
helpers, signature size preflight and fixed term measurement ceilings. Keep
checked representational binder/universe arithmetic and existing typed shape
checks. No new matching cases. The legacy structural implementation and its
runtime consumers remain named migration debt until actual upstream
SolverDelegate/candidate/member/dictionary integration replaces and removes
them under E-R5-014. This repair does not promote legacy trait solving.

Cold cost: existing traversals/owned substitution remain proportional to
visited terms (existing cloning costs remain); remove redundant accounting
walks. No new dispatch work, source preparation, IO, namespace census, body
copy or native capability. OS owns allocation limits. Existing public heap
measurement APIs outside this slice remain separate accounting repair debt;
do not claim repository-wide removal or a measured speed improvement.

Acceptance: preserve baseline refusal, async guarded build plus the unchanged
boundary control after removal, existing transaction/binder/shape controls,
and actual unchanged stock runtime probes. Record incomplete captures and
unsupported features separately. Perform a separate source review against
all eight recurring modes at the exact final source fingerprint. No feature,
runtime-suite or JIT completion follows from a green host metadata test.

Reservation integrity: actual same-root existing owners are LOCAL-037 for
signature_subst/generic_boundary_tests, LOCAL-044 for vm/frame.rs, LOCAL-046
for traits/term.rs, LOCAL-047 for traits/unify.rs; LOCAL-049 retains subst.rs.
Original path reservations are preserved and shared interfaces recorded, not
duplicated claims or concurrent writers. Initial planning overlap rejection
and corrected passing check are retained in evidence.
