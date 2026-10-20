# M020 independent negative-bound source review

Final scope decision: negative parser activation was withdrawn before validation
because direct conditional-impl membership/default-body/Copy/Drop availability
is still missing. Two negative control candidates remain explicitly held and
unexecuted. V512 validates only six existing source/constructor regressions and
four unchanged runtime witnesses, at the fingerprint recorded below. Shared
guard candidates do not establish negative language support or complete positive
consumer closure. Earlier narrower guard-safe conclusions are superseded by
the direct conditional-impl review and hold.

Reviewer `/root/wf_validation_review`; baseline
`49cffe1587e2d931efe1c7c6b44f07fad97490af`, LOCAL-049 / WT05-02 / T012 / M020.
Read-only production review while source is mutable. No Cargo, compiler or test
execution. Final source fingerprint, controls and closing guards remain pending.
The unchanged witness is tests/ui/traits/negative-bounds/supertrait.rs at the
existing exact Rust pin. Its check-pass directive is source provenance, not a
runtime or compiler acceptance result.

## Grammar and shared source collection

Inspected vendor/ra-ap-parser/src/grammar/generic_params.rs::type_bound and
path_type_bound against pinned rustc_parse/parser/ty.rs::parse_generic_bound,
parse_trait_bound_modifiers and parse_trait_bound. The new BANG consumption stays
inside the same TYPE_BOUND marker. Plain !Trait and parenthesized (!Trait) thus
retain a direct BANG token; outer parentheses do not create another bound AST.
The same bound parser serves generic parameters, where clauses, supertraits,
associated output bounds and impl/dyn bound lists. No second parser is added.

The pinned parser reads an optional higher-ranked binder before modifiers;
for<'a> !Trait retains both pieces but triggers a separate compiler diagnostic.
The existing common collector returns HigherRanked before its qualifier guard,
so that admitted shape remains pending rather than positive. The pinned parser
also recovers !for<'a> Trait with a diagnostic; it is not a successfully compiled
syntax requiring an additional valid grammar production in this bounded change.
Do not implement those static diagnostics as interpreter runtime refusals.
Sequential ? and ! admission is more permissive than the compiler's exclusive
polarity grammar; the direct BANG guard still prevents a positive recipe.

`source_types::actual_declaration_bound_syntax` preserves actual bound nodes,
subjects, spans and where binders. The shared source_predicates::prepare_bound_rows
now tests a direct BANG before subject/trait resolution and emits existing
Pending(QualifiedBound). The higher-ranked guard still takes precedence. This
serves Q78 own recipes, Q82 output/parent recipes and general impl predicates;
it cannot silently turn a negative trait into Implemented. Q84 sees pending
written source input and remains WrittenRecipe pending, never complete empty.

## Blocking positive-reader completeness findings

Admitting new syntax in the common parser exposes other existing direct AST
readers that bypass prepare_bound_rows. At this review they ignore BANG and can
use a negative bound as positive member/candidate/equality evidence:

- source_types.rs::projection_bound, including its existing token-list fallback;
- source_types.rs::actual_bound_equation_candidates;
- declaration_nominal_types.rs::parameter_traits;
- associated_constant_applications.rs's actual declaration bound selection;
- member_index.rs::blanket_bounds;
- source_types.rs::resolve_actual_type's impl/dyn principal-bound selection.

The first group filters only question marks/lifetimes, then reads bound.ty().
The impl/dyn route simply selects the first path bound. The bound's path is now
present for !Trait, so Q78's pending guard does not protect these paths. Findings
were sent to the lead before validation. A common retained-AST positive-bound
polarity classifier must exclude negative rows everywhere positive evidence is
consumed. Existing broader qualifier/binder admission is separately named debt;
this bounded repair does not add compiler static checking. Consumers must retain typed incompleteness when
skipping a row would otherwise prove absence or complete candidate membership;
do not replace negative evidence with a successful empty answer. In particular,
actual_bound_equation_candidates.owner_complete currently checks declaration
identity/generic layout, not whether every semantic bound was admitted.

The lead's first follow-up added a shared polarity classifier and guards in the
listed frontend consumers. Rereview found two further gaps before freeze:
syntax/source_constraints.rs and syntax/let_annotations.rs also choose a dyn
principal from bound.ty() and need the same negative admission. More critically,
the proposed blanket `None` sentinel is accepted by existing readers using
Option::is_none_or as automatically satisfied. It does not preserve negative
incompleteness and would still admit an unconditional candidate. The lead was
sent exact readers (`trait_provided_candidates`, `implements_depth`) before any
build. A repaired representation must be checked at every blanket dispatch reader,
not inferred safe from its producer's comment. Final repair review is pending.
The existing outer blanket=None already denotes unavailable applicability and
primitive_candidates preserves it as unresolved=true; inner Vec<Option<Digest>>
None has a different permissive meaning. These two domains cannot be conflated.
The shared classifier's direct-token inspection correctly avoids treating a
nested Never type or const-expression BANG as the outer trait-bound polarity.

The repository-wide pass also inspected legacy token consumers rather than
stopping at AST TypeBound readers. binding_constraint_callables.rs::fn_sugar
searches for any Fn/FnMut/FnOnce/fn token, dropping its prefix; !Fn() -> R can
therefore become positive FnBound through bound_from_tokens. Separately,
binding_constraint_bounds.rs::classify_bound treats !Trait<Assoc=U> as ordinary
Bindings, and inside_bounds_inner calls relate_binding to impose a positive
projection equality. Both were reported before freeze. Their admission must
come from genuine source polarity/completeness or return existing typed/deferred
unsupported results; do not create another bound parser or accept absence.
binding_constraint_generics::bound_paths, by comparison, retains `!` in its
lookup path and does not silently strip it into a positive Trait identity;
that token reconstruction remains named legacy debt, not the replacement route.

The later common helper moved to syntax/source_bound_polarity.rs.
positive_principal now scans the entire actual bound list, refusing a negative
row even after an earlier positive principal; all three impl/dyn readers reuse
it. Blanket construction now returns the existing outer unavailable marker for
actual negative conditions. Rereview of these changes closes the two specific
gaps above; there is no negative-as-positive fallback through the inner sentinel.

For the raw-token interfaces, the lead proposed demand-time actual-AST admission
in ParsedFile::function_signature: refuse unrepresented negative bounds in the
actual function header and actual associated trait/impl header before creating
legacy token metadata, excluding bodies and other members. Call-site review
confirmed every production fn_sugar/bound_from_tokens/classify_bound flow first
fetches actual QueryKey::FunctionSignature through method_signature or a direct
query. No bypass was found. A common explicit unsupported signature-metadata
result is therefore a sound staged hold until these readers migrate to genuine
bound AST/query inputs; it is not a compiler validity check or a negative solver.
Zero-argument initializer/discriminant signatures do not supply these generic
FnBound parameter paths. Actual implementation of that admission still needs
final review; direct negative Fn/equality interfaces remain unexecuted unless a
later actual-source control explicitly reaches them.

The first negative_in_header implementation filtered only direct child bodies,
then traversed every descendant of other header children. Review found that it
would inspect nested executable/local declarations inside a const-generic or
array-length block under a parameter/result/header node. That can reject an
outer signature due to an unentered cold item's negative bound. The lead was
notified before freeze: every-depth preorder subtree pruning is required for
BlockExpr/AssocItemList, not merely direct-child filtering. The actual source
signature boundary remains the right place; this traversal detail must preserve
its promised demand scope. No runtime observation is claimed for this edge.

The final pre-freeze source pass reread the corrected preorder implementation:
it now prunes those subtrees at every depth. ParsedFile::function_signature
checks the actual function and associated_owner before generic/signature token
lowering and returns the exact unsupported semantic-transport diagnostic. Existing
initializer/discriminant paths remain unchanged. No remaining negative-as-positive
bypass was found in the repository AST/token reader pass, within this scoped
admission contract. Full replacement of token readers and qualifier/binder
transport remains debt; source inspection does not establish their execution.

The new second control uses unchanged negative-bounds/associated-constraints.rs
and its stderr provenance, demanding just the five real function signatures in
twelve edition/origin worlds. It checks actual retained negative bound tokens,
independently available source DefinitionHeader metadata, exact unsupported
signature query outcome, error memo reuse, header currentness and no main/body/
thunk/pattern demand. It does not assert rustc's static diagnostics. The existing
supertrait control checks 24 real trait declarations across twelve worlds. Both
controls are source-reviewed here and await actual terminals. Parenthesized,
higher-ranked, associated-owner, nested-block pruning, negative blanket/dyn and
standard-macro variants remain source-reviewed but separately unexecuted edges.

## Further direct conditional-impl blocker before parser activation

The lead requested a final reached-membership audit, which found a broader
remaining prerequisite. Blanket outer=None does not protect an impl whose self
type is nominal or builtin: MemberIndex::implements_depth accepts direct trait
IDs without consulting conditional bounds. copy_types/drop_types likewise
publish direct nominal membership; RunMachine::ensure_drop_inventory installs
these ordinary entry facts, and GuestStore::intrinsic_read_mode uses Copy
membership for reached operations. Incremental LocalContainer publication has
the same Drop fact boundary. run_methods' Receiver branch also reads implements
before selecting the source Target projection.

This is not only a marker issue. trait_provided_candidates can choose a default
trait method from a negative-conditional impl; the selected function signature
belongs to the trait, so its header does not contain the impl's negative bound.
The function signature guard therefore cannot close every candidate path.
These actual bypasses supersede the earlier narrower statement that the scanned
AST/token readers were guarded. No build or passing claim was made between the
passes; the lead received the exact call sites before freeze.

The correct prerequisite is typed conditional-impl applicability on demanded
candidate/membership domains, shared by direct/builtin/blanket and runtime marker
facts. An unsupported negative condition cannot be positive evidence, false,
empty membership or a valid default-body candidate. Globally rejecting a member
index due to unrelated cold negative impls would violate demand scope; globally
setting Copy inventory unknown would also refuse unrelated nominal reads.
Per-type availability could be installed cold and checked by reached operations,
but the current bool membership and collection APIs do not express it uniformly.
Holding parser activation is preferable if that common representation/propagation
prerequisite is too broad for this bounded slice. Full negative semantics are not
required merely to report typed unavailable source input. Parser activation and
source-completeness claims remain held pending the lead's chosen common repair.

Concrete source-shape counterexamples used in this review (not new substitute
fixtures or executed compiler/runtime tests) are:

- `impl<T: !Bound> Trait for Wrapper<T> {}`: implements_depth's direct
  trait-ID match treats the unresolved condition as satisfied. When Trait
  provides a default method and this impl supplies no override,
  trait_provided_candidates returns the trait method's function ID; its
  FunctionSignature has no corresponding impl header and bypasses the negative
  signature hold.
- A nominal Copy impl whose actual header includes `T: !Bound` (alongside
  whatever genuine Copy/Clone requirements its declaration needs) is added to
  copy_types by self_nominal alone. ensure_drop_inventory then installs the
  nominal identity, and GuestStore may treat it as Copy based on reached fields
  without proving the additional negative condition.
- A nominal Drop impl with such an unrepresented condition likewise enters
  drop_types/drop_functions and incremental LocalContainer Drop publication
  without a per-application condition result. A Receiver impl uses the same
  unconditional direct-membership query before its Target projection demand.

These expose missing source authority/applicability in the actual inspected
query routes; they are not measured misexecutions or assertions of rustc static
acceptance for an arbitrary standalone snippet. The unchanged official source
witnesses remain the primary reproducible inputs. The lead withdrew parser
activation specifically because the narrower guard-safe opinion did not cover
these common direct-membership/default-body/runtime marker consumers.

## V511 source freeze: parser activation withdrawn

The lead withdrew the parser change before any build, test or runtime execution.
The vendor generic_params.rs diff is empty. Shared polarity/principal/signature
guard candidates and two negative source-control candidates remain retained;
the tracked negative-bound-source-controls selection explicitly marks both
negative controls held_unexecuted. The six active selectors are existing
Q80/Q82/Q83/own-hierarchy and constructor regressions. Passing those would not
establish negative grammar, member applicability or runtime support.

The independent review is bound to frozen fingerprint
`f3d1221fb80ce91078adaaa07b8496cc86d3b56b13ff634a85ffe70737306f95`,
HEAD `49cffe1587e2d931efe1c7c6b44f07fad97490af`, 8,713 guarded files. The
captured delta from V510 contains the named guards, HRTB assertion/control
changes and exact incremental fixture import/manifest; no vendor parser change.
The retained negative controls are not run or counted as passes. Common typed
conditional-impl availability, selection and reached Copy/Drop propagation is
the next architecture prerequisite. Host panic profiles remain unchanged;
future relevant fact closure/preparation ahead of callbacks is the current
direction, with no approval or patch for host unwind policy.

Before running controls, independently verified imported supertrait.rs,
associated-constraints.rs and associated-constraints.stderr byte-for-byte against
the exact upstream Git pin, including manifest SHA256/size identities. Root
LICENSE-MIT, LICENSE-APACHE and COPYRIGHT likewise match those Git objects and
their manifest rows. This is source/licensing provenance only, not executed
compiler diagnostics or negative language acceptance. Policy/raw port sources
are unchanged from prior audited evidence; no redundant full port audit is run.
Builds, selected controls/runtime and final guards remain pending for V511.

V511 then terminated at the library no-run build with exit 101: E0433 at
syntax/let_annotations.rs:328, where `super::source_bound_polarity` does not name
the actual common syntax module from that reader's nesting. No CLI/digester
build, active control, held negative control or runtime case executed. The
module path mismatch escaped the bounded source pass; compiling guards is not
semantic acceptance. After-build and after-runtime source/HEAD guards both
terminated 0 with zero changed files. The explicitly old V510 CLI and digester
remained unchanged, including a separate digester closing comparison. All
processes were terminal and source/Cargo ownership was returned before repair.
V511 input/licensing provenance passed independently as recorded above.

V512 is separately bound to source fingerprint
`ebea0de06be9db158f640f7d6708335b1cfa9236a2a55edc481c8a2d4d215ec9`,
at the same HEAD and 8,713 guarded files. Its exact manifested delta from V511
is two absolute helper module-path corrections in syntax/let_annotations.rs and
syntax/source_constraints.rs. The common helper/consumer source review and
documented conditional-impl hold are otherwise unchanged. The parser remains
on its prior grammar and both negative controls remain explicitly unexecuted;
build/control/runtime/closing terminals are pending for V512.

V512 final terminals: library no-run 0 (34.06 s), CLI build 0 (27.87 s),
digester build 0 (0.55 s). All six active exact controls terminated 0, taking
29.551/29.006/32.206/28.682/50.145/23.543 s. Actual completed world rows were
12/12/12/12/12/4. Observations: 36 own clauses/12 projected subjects; 12 written
output/12 default recipes; 24 parent/12 own clauses and 24 Q83 associated
declarations; 12 HRTB own-ready-empty/parent-pending worlds; 264/264 and
1,624/1,624 operations/completions in the constructor controls. Every selected
body ran once with unchanged inputs. The HRTB exact Source(DefaultBounds(
WrittenRecipe)) classification is now measured at V512; V510 remains a failed
assertion receipt at its own fingerprint. This does not complete compiler
parent/default query semantics.

The two held negative control bodies were not selected or executed, and negative
parser activation remained withdrawn. No negative fixture, language, compiler
diagnostic, member applicability or runtime acceptance follows from these six
neighboring component passes. The actual original licensing/source proof is
independent of execution.

The four unchanged owning-2015 runtime witnesses remained three contract passes
(associated constructor, generic-derived-type, empty-generic-brackets; all
13 checks true each) and repr128 executor fault LRR-E-OP-TYPE, `operator requires
supported concrete primitive operand descriptors`, without passing runtime
checks. Batch exit 1; no timeouts/output limits/native comparison/JIT. There is
zero new runtime gain and no full-fixture/diagnostic/JIT gate acceptance. Report:
/private/tmp/lrr-v512-conditional-applicability-hold-runtime/report.json.

After-build and after-runtime guards both terminated 0 at the recorded V512
source fingerprint/HEAD/8,713 files, zero changed inputs. Actual Cargo JSON test
SHA256 `4a67b36099dc091d9da4dd52d4b80f4d692595d421bebd528fa590f842064338`
and fresh CLI `3c501637901d6222258bab51aa855a4cbe62a599062a94c9d0ce6250c1329eab`
matched their after-build receipts. The separate fresh digester closing hash
matched `4069900bad4db1e1cd27f01a3baff0f9c025ed8d48250d6567639a42b18195d8`.
All validation processes were terminal and source/Cargo ownership was explicitly
returned before any repair/checkpoint. Raw logs and parsed actual counts remain
disposable under artifacts/validation-current-v511 and -v512; maintained tracked
tools/selection reproduce the attempts. The conditional applicability/common
query availability prerequisite and broader eight-mode architecture matrix
remain held as described above.

These legacy readers remain migration debt, not permission to create a second
negative solver or enlarge source-specific dispatch. The actual pinned solver
still owns negative semantic reasoning when its genuine inputs are eventually
available. This slice only establishes honest source admission and typed holds.

## Eight-mode independent review and evidence boundary

1. No assertion/expression evaluator is introduced; source predicates use the
   existing ordinary bound collector. The legacy readers above must not bypass
   its semantic admission merely because parser recovery now succeeds.
2. Common TypeBound syntax is retained for all origins; no literal/break-only
   macro route is added.
3. Polarity is selected by actual source tokens, not assertion/member/fixture
   names. Blanket/member readers' positive interpretation is a blocking gap.
4. Original, user-emitted and wrapper syntax join the same parser/source queries;
   generated source needs its existing actual header/scope/currentness receipts.
5. No pattern engine, body checking or executable cold-arm preparation is added.
   Unsupported negative transport is metadata pending, not a static rejection.
6. Cold retained plans/AST nodes are reused. A direct-token scan is O(bound
   children); collection is O(requested declaration bounds), with no installed
   runtime reparsing. Existing token-list fallback is named legacy debt.
7. The proposed admission guard needs no namespace census and must run before
   trait resolution for unsupported polarity. Real source memo/negative lookup
   dependencies remain authoritative; unrelated invalidation is not measured.
8. No BodyIndex clone or repeated whole-file lookup is introduced. Existing
   indexed declaration/bound nodes and query Arcs provide ownership.

The negative source control is owned by another agent and remains unexecuted
at this record. Required evidence is actual unchanged source and direct BANG
retention across four editions and three origins, exact typed pending at Q78/Q80,
memo/currentness/cold-path checks, plus affected prior controls after final
freeze. Parenthesized/binder cases are source-reviewed only unless separately
executed. Full negative query semantics, solver root, runtime, standard-macro
matrix and interpreter/JIT gates remain held.
