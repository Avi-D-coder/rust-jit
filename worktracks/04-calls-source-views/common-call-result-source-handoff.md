# Common reached call/result source ordering

Root, LOCAL-024, WT04/WT08, T042/T043/M014, macOS/all official editions.
Bounded successor to nominalidentity59; no task or audit closure.

Actual witness: immutable nominalidentity59 retained-reference bank failed
`common_reference_results_forward_real_caller_provenance_after_cleanup_all_editions`
with `LRR-E-UNSUPPORTED: source view requires further value-family integration`
at guest span24..36 (`8bac89/1`). Its nested `read(take(&value))` supplies a genuine
reference result expectation to `take`. Root and validator source review found
`engine/run.rs` prepares a primitive ArgumentType before reading an existing
`Frame::source_view`, in both selected argument preparation and reached call
dispatch. The evaluated-argument path already reads the actual source first.
`engine/run_return.rs::begin_return_cleanup` repeats primitive-before-source
ordering for non-reference/rigid result binding. This rejects source types before
the ordinary source contracts can check them.

Further source inspection found that `run_argument_hints::runtime_hint` correctly
publishes only the descriptor for a call-expression reference argument; it cannot
mint a Local region before the actual borrow/return is evaluated. Source-first
ordering alone therefore cannot repair this witness. Preserve descriptor-only
expectations separately from symbolic source hints. `call_result_source_hint`
validates the descriptor but leaves the symbolic hint absent when no actual
nonprimitive source exists. The checked declaration/result boundary still checks
its known result descriptor against the caller slot before publishing a call;
the actual returned source/provenance is admitted by the existing return path.
This is deferred source preparation, not a fabricated region or accepted mismatch.

Extend the evaluated argument/return entries with one private ArgumentType constructor that uses
the actual admitted source when present, and preserves the existing primitive
conversion only when no source exists. It cannot infer nominal/reference source
identity from a TypeId or layout. Original, retained and generated callers use
the same path. Preserve the existing exact source substitution, descriptor,
reference provenance and currentness checks in call contracts and return cleanup;
no macro-name or source-origin branch, new evaluator, parser or static check.
No duplicate language route is added; remove duplicated source-after-primitive
construction at both result-hint sites and the reached argument/return sites.

Preparation cost remains one existing slot source lookup and bounded TyTerm
clone. No namespace scan, source walk, executable cold-body demand, new query
framework, world invalidation or BodyIndex copy. The separate namespace witness
and product binding-cache repair remain open, not replaced by this change.

Evidence required: sole validator executes the unchanged failing result-route
control on exact nominalidentity59+only these two source leaves; preserve all
source-origin/all-edition/effect/cleanup/cold-body assertions. Then existing
ordinary/reference/rigid call-return controls and the current milestone as scope
allows. Official unchanged `regions-addr-of-ret.rs` and
`regions-infer-borrow-scope.rs` remain the upstream language witnesses; their
incomplete full runtime dependencies remain blockers. This source repair and its
LLR route checks do not establish a complete upstream fixture pass.

Eight-mode source review: same common call and return services (1/2); no numeric
or method context rule (3); all origins follow the same source-first admission
(4); no pattern or cold-arm execution change (5); no added planning (6); no added
namespace/currentness work (7); no immutable body clone/full-file search (8).
All MA001-MA008 architecture findings remain open. Rust execution is pending
until actual terminal evidence for this exact successor is collected.

## Follow-on actual ordinary-path witness

Frozen61 compiled but the exact reference-result control failed (`856a0f/1`):
`floating_expected_source` rejects a nonprimitive expected source without its
descriptor, before `RuntimeMacroContext::with_floating_site` determines whether
the current path is even a floating-constant candidate. No float literal is in
this witness. Root moves the existing expected-context read behind that same
service's actual qualified-path source selection via a synchronous read-only
callback. An unrelated path then returns None through the existing selector;
an actual selected floating path still propagates the exact context error and
retains all ordinary namespace/source/descriptor checks. No caught-error success,
new float/source type inference, alternate evaluator, source-origin rule or
per-phase parser is introduced. Existing floating service planning/revalidation
cost and origin gaps remain explicit debt; this does not close their audit.
Validate the same reference-result control and existing floating positive/negative
source authority controls on frozen61+only these two floating dispatch leaves.

## Common physical parameter storage prerequisite

The wrapper control now passes all5 source forms across4 editions on frozen74.
Core nominal/interior controls on frozen78 still fail with VALUE-REPR. Validator
mapped all5 actual spans to ordinary calls and identified
`GuestStore::new_parameter_cells` demanding `primitive(target)` for every reference,
after `Frame::initialize_parameters` has already checked actual packet types,
source views, nominal reference environment/descriptor and required source targets.
See read-only diagnosis `14f54d/0`; this is a confirmed reachable rejection, not
proof it is the only remaining failure.

Root replaces this low-level primitive/String whitelist with registered descriptor
validation. Empty physical parameter cells do not grant language/source authority
or materialize any value family. The sole caller still performs exact source and
argument-batch checks before allocating cells; packet delivery still validates
actual values, types, reference records, provenance, mutability and ownership.
Unknown descriptors, quota/count/base overflow and allocation refusal remain
preallocation checks. No scalar/reference/aggregate validation is weakened and no
type identity is inferred from layout. All original/generated/retained calls use
the same physical allocator; unsupported source contracts remain with ordinary
source admission. This removes a misplaced language-profile gate rather than
adding another source-specific whitelist.

Frozen78+only this RuntimeStore leaf requires existing core8 and source/descriptor
forgery negatives first. No tests, configured upstream pass or task closure can
be inferred from the source change. Retained constructors and body context/query
ownership remain independent blockers. No new parser, evaluator, planning,
namespace scan, cold-body demand, BodyIndex clone or unsafe code is introduced.
