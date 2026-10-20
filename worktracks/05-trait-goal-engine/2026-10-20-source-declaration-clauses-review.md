# M020 independent source-clause bundle review

Reviewer `/root/wf_validation_review`; implementation baseline `8c764548b1`.
This record retains the initial read-only contract review and separately bound
V504/V505 validation receipts below. The reviewer owns this new worktrack record
and selection documentation, plus sole asynchronous validation after the lead's
explicit freezes; no production source or compiler original was edited.

## Pinned query contracts and API boundaries

Inspected exact pinned originals at
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`:

- `rustc_middle/src/ty/generics.rs::GenericClauses` and
  `ty/mod.rs::InstantiatedClauses`: parent-first recursion, same full argument
  list through parent calls, aligned clause/span vectors and unnormalized
  clauses. Ordinary instantiation uses the existing EarlyBinder/ArgFolder;
  identity instantiation preserves raw parent source identities.
- `rustc_middle/src/ty/context/impl_interner.rs::clauses_of` flattens the
  compiler's full GenericClauses using instantiate_identity. `own_clauses_of`
  instantiates only that full query's current-definition clauses. Neither is
  simply the written bounds retained by Q78.
- `rustc_hir_analysis/src/collect/clauses_of.rs::clauses_of` appends inferred
  outlives to explicit clauses and adds a trait identity Self:Trait clause.
  `gather_explicit_clauses_of` includes implicit sized/default-trait bounds and
  ConstArgHasType for real const parameters as well as written requirements.
  Trait `explicit_clauses_of` filters only applicable own-associated identity
  projections into item bounds, not every projection subject or supertrait item.
- `collect/item_bounds.rs` includes declaration output bounds and applicable
  remapped parent-trait where-bounds with implicit sized/default-trait rules.
  item_bounds elaborates; item_self_bounds elaborates and filters only self;
  item_non_self_bounds is ordered set difference. These are separate producers.
- `outlives/mod.rs::inferred_outlives_of` has a genuine empty result for ordinary
  functions by role. ADTs, checked aliases and special generic-const default
  contexts have separate inference demands. Absence of written lifetime bounds
  cannot establish empty inferred outlives for an ADT.

A source-explicit-own bundle may be useful without establishing any complete
compiler clause family. It must keep its query identity distinct and retain
typed pending for unproduced inferred, implicit, item, super or implied inputs.
One convenient list cannot populate every PredicateFacts field. In particular,
associated `type X: Bounds` output bounds are not its where-clause obligations.

SourceParameter includes source-owner identity. Consequently parent identity
clauses and clauses ordinarily instantiated with actual child identity arguments
are distinct observations. Parent-first indexes provide substitution, not a
license to guess source ordinals or claim the preserved parent's raw parameter
identity is already the child's. Actual declaration/generic/parent receipts must
authenticate this transport. Lexical ScopeKey.owner is not automatically the
generic clause parent: nested free items do not inherit arbitrary enclosing
function parameters or clauses.

The prior factory observation remains relevant: a function serving only as the
owner of a source environment does not thereby require a FnDef type/signature,
nonzero function variance, CTFE or transmute fact row. Complete relevant impl
membership still matters when upstream actually evaluates a trait goal. The
existing Cartesian associated-leaf admission and missing root/runtime dictionary
integration remain separately held; this component does not relax ReadyProvider.

## Generic instantiation port source pass

Inspected new `generic_clauses.rs` and `tools/upstream_generic_clauses.py`.
The generator hashes the complete already-retained originals and extracts all
six instantiation methods, plus the complete InstantiatedClauses storage,
constructors and owned/borrowed iterators. It substitutes concrete shared IR,
separate data/context lifetimes and a typed immutable ClauseParents lookup.
The same full argument list reaches the parent; spans and parent-before-own
ordering are retained. Args.items() matches the pinned copied iterator API;
model Span's existing atomic folding contract preserves metadata through unzip.
The borrowed data lifetime supports an owned immutable map without leaking data.
ExplicitOwnClauses and FullClauses are distinct type markers; this port does not
produce compiler-complete clauses or read a partially prepared SourceFacts map.
No mechanical or lifetime defect was found in the read-only pass. Compilation
and actual-source parent/child controls remain unexecuted until freeze.

The production Q80 bundle, associated predicate producer, old cfg helper
migration and their currentness/selection controls are still being implemented.
Their actual paths and changed call sites require the final source pass. The
unchanged repr128 own-source bound witness and associated Tr::T parent-context
witness are metadata controls, not new upstream main/runtime passes.

## Eight-mode review status before freeze

1. No expression/assertion evaluator appears in the generic instantiation port;
   the arriving source bundle must use the common predicate encoder.
2. It reads source-independent clauses rather than executing macro output.
   Original/user/wrapper source admission still needs actual control terminals.
3. No method/numeric/fixture policy exists in the port; source-role and generic
   proofs must remain ordinary metadata rather than name-based semantic rules.
4. Explicit query markers prevent a new source-only compiler query substitute.
   Generated associated joins remain part of the arriving producer review.
5. No patterns or executable arms are prepared by instantiation. The production
   bundle must retain declaration/signature/body demand separation.
6. Instantiation traverses retained clauses and substitutes actual arguments;
   it does no parsing or source-plan reconstruction. Owning memo reuse remains
   to be checked in the production bundle and controls.
7. ClauseParents is an immutable map lookup, not a live namespace or source query.
   Actual dependency/currentness closure is the source bundle's responsibility.
8. No BodyIndex cloning or whole-file node search occurs in the port. The
   arriving producer must retain source/query Arcs and direct syntax locations.

All eight conclusions are preliminary and component-scoped. Exact source-bound
review, compilation, cold-path/currentness controls and final guards are still
pending. Full source factory, actual upstream root/member dictionary integration,
runtime, diagnostic/full-fixture and JIT gates remain held.

## Arriving producer source pass

The associated Q78 pass now retains actual child/parent DefinitionHeader and
DeclarationGenericTypes roots. It verifies source/header receipts, parent header
tokens, inherited count and implicit Self, and each inherited parameter's kind,
name, bounds, defaults and const Type before making the source rebase map.
Trait Self uses the real child's inherited slot. Impl Self resolves the retained
parent AST with the existing WrittenTypeDeclaration rebase bridge; it does not
reparse an impl header string or manufacture a function. Own parameter spans
come from the child GenericParamList, and the common collector keeps own clauses
separate from parent requirements and associated output bounds. The two new
source controls cover real aliases/GAT contexts and the HRTB parent-demand
boundary. Their epoch revocation checks are not separate source-withdrawal proof.

An initial Q80 draft globally required its parent's recipes even for its own
encoder. Review identified that as an unnecessary demand. The lead repaired the
common boundary before validation: Q80 now owns only source requirements and
actual parent metadata, while distinct Q81 SourceDeclarationClauseHierarchy
explicitly requests the parent recipe closure. A parent HigherRanked pending
recipe cannot contaminate an otherwise complete child's own requirements.
The unchanged imported assoc-type-projection-method.rs Collection::my_iter is
the minimal witness: own requirements are empty; its parent Collection has the
real HRTB requirement. This preserves metadata incompleteness without forcing a
return Type, function signature, body or parent requirement when unused.

`source_predicates::{require_recipe,encode_recipe}` now serve declaration and
existing impl clauses. Their behavior matches the removed impl loops: relaxed
requirements retain dependencies but emit no clause; implicit Sized needs actual
identity and own span; pending reasons remain typed. The old cfg projected-bound
helper now captures CurrentSourceDeclarationClauses and calls the production
encoder instead of preparing/encoding an independent test-only bundle.

Q80 encoding checks source world/configuration root and actual generic source
metadata; it reads retained recipes and bindings only. Q81 encoding builds owned
context-bound rows and borrows ExplicitOwnClauses views through the immutable
ClauseParents map. Current source guards retain actual memo dependency closure;
they certify source plans, not an independently completed runtime context or
future full DefinitionFacts installation. Database key/value encodings use
distinct tags 80 and 81, with provider and existing loader/expansion metadata
classifications registered for both. No new accounting or guest limits appear.

The source passes found no new evaluator, scalar macro execution, contextual
method/numeric inference, original-only refusal, pattern/static/body checking,
installed parsing/planning, runtime namespace census or BodyIndex copy. Cold
cost includes demanded generic/parent metadata, bound syntax and retained parent
clause chains; per-argument pinned substitution is semantic IR work rather than
source reparsing. Overlapping cold closure sharing/performance, full namespace
negative/currentness matrices and standard/procedural macro execution remain
unmeasured. Final source fingerprint, parent test review and actual terminals
remain pending the lead's explicit global freeze.

The exact six-selector selection is
`2026-10-20-source-clauses-controls.json`: pinned parent instantiation and the
own/hierarchy HRTB demand boundary first; the two associated Q78 controls; the
existing repr128 projected-bound control migrated to production Q80; and the
existing impl-clause control exercising the shared recipe encoder. The runtime
regression replay remains the same two unchanged owning-edition cases,
repr128.rs and generic-derived-type.rs, with embedded facade and a fresh report
directory. The new generic-clause mechanical audit is separate from the six
prior audits; explicit bootstrap Git-object verification remains required.

## V504 exact source review and failed build

The completed independent source pass is bound to
`a1bc093035a28b7a341339f9f697a70138805fcfc4ddbcf235ca83393df54452`
(8,700 inputs), HEAD `8c764548b147240f631f29f1e49cf4a1aea67fff`.
The actual changed routes reviewed above are Q78 associated source metadata,
Q80 own source clauses, Q81 explicit hierarchy, shared impl/declaration recipe
transport and the complete pinned generic instantiation port. The previously
overbroad Q80 parent requirement was removed before this fingerprint. The old
cfg projected-bound preparation/encoder route is removed; the host helper now
observes production Q80. New source-parent/hierarchy controls independently
select actual StreamEnumerate impl/GAT declarations, preserve raw parent
parameter identity under identity instantiation and observe real child owner
rebasing under ordinary pinned instantiation. They assert two parent clauses
before one own clause, aligned actual spans, own iterator behavior and zero
query trace during context encoding. The HRTB control checks no parent recipe
demand until Q81 is explicitly requested, then exact typed parent pending.

All eight source-review conclusions were rechecked against this fingerprint.
They cover the actual entry points and remaining exceptions listed above, not
runtime matrix closure. Test-only parent()/fixture_world() visibility is shared
between sibling host modules; the Q80 source reader is crate-visible. The
OutlivesClause representation matches the pinned TypeIR tuple struct. Ordinary
compiler-complete own/full/item queries, implicit/inferred clause completion,
function/generic macro identity production, full source-fact publication and
runtime/member/JIT integration remain held. No callback readiness condition was
relaxed. Standard/procedural macro and complete runtime architecture matrices
were not exercised by this source review.

The sole validator launched the direct-checkout offline locked Rust 1.99.0
library no-run build asynchronously. It terminated 101 with four mechanical
diagnostics: the associated host control omitted the StableEncode trait import
for DefKey.fingerprint; the hierarchy stable encoder attempted to encode the
QueryValue wrapper instead of its retained hierarchy payload; and two lifetime
diagnostics came from the cfg helper's independently elided Clause/Provider
lifetimes. No source repair was performed by the validator. CLI/digester builds,
all six selected bodies and both runtime cases were unexecuted at V504.

All seven source audits terminated zero using `/opt/homebrew/bin/python3.12`:
full inference, bootstrap, language vocabulary, transmute, nominal type,
variance and generic clauses. Separate bootstrap `--verify-upstream` also
terminated zero. These audits are provenance/source evidence, not compilation
or runtime acceptance. Before, after-build and after-runtime receipts matched
the exact source fingerprint and HEAD with zero changed inputs, after all
processes were terminal. Failed-build binary receipts identify only the old
V503 CLI `cf677ba379f98c7e3ff1313ed9463369cc550c319895aa8b581d459a636a38ce`;
it is not a V504 binary. The old V503 digester
`0288fe353b015782dec8c6d99e82e41fb34441200193e97b6eae0eccf0c7bd77`
also matched its prior closing receipt. No fresh executable or passing V504
control/runtime result is claimed. Source, HEAD and binary ownership were
returned after those closing guards. Raw logs and receipts are in
`artifacts/validation-current-v504`; required tools and exact selection remain
tracked outside that disposable directory.

## V505 exact mechanical delta review

V505 is separately bound to
`24d558cff3b26279ee38bd0a05528c3ecbe380d99dfb59cae7fe3eaf5aed3bbf`
(8,700 inputs; same HEAD). Captured-manifest comparison found exactly two
changed inputs: upstream_associated_predicates.rs and
source_declaration_clauses.rs. The host control imports StableEncode; Q81's
stable encoder matches the retained hierarchy payload before encoding it; and
the cfg observation helper's parameter function couples Clause and Provider
with one named context lifetime. These repairs change no query demand, source
semantics, clause ordering or instantiation policy. All eight source-review
conclusions carry with the documented holds. V505 compilation/controls/runtime
terminals remain separate and pending.

The new Q81 controls advance epoch first and subsequently confirm the same
receipts remain revoked after source withdrawal. Independent source-only
withdrawal for Q81 remains unexecuted. The existing repr128 Q80 control withdraws
sources without first changing epoch; a passing terminal would establish that
distinct own-bundle case. Neither observation should be generalized to the
complete currentness/negative-dependency matrix.

## V505 compilation and runtime receipts

The direct-checkout Rust 1.99.0 offline locked library no-run, CLI and xtask
source-digester builds all terminated zero. The library build took 38.71s and
the CLI build 35.48s. Cargo JSON identified the actual test executable
`target/debug/deps/lrr-df1e3f56d083c63c`, SHA256
`074ffdcd73a958c49d7dd88386f13238ffe5674c034db20030810ad4ac525664`.
The fresh CLI SHA256 is
`a3c6b5af08aedc91fda1d8671910b4ebf84bc6a3d59145c220faf68458e84498`;
the fresh digester SHA256 is
`6e3677fd0fed33b77a1a4de7972d9449b798d60a371215c9f213c046b508265e`.
The after-build guard matched all 8,700 source inputs and HEAD.

All seven pinned source audits and separate bootstrap `--verify-upstream`
terminated zero with `/opt/homebrew/bin/python3.12`. The latter verified local
Git objects at the pinned upstream revision: 9 crates, 227 files, 3,703,000
bytes and zero retained-original Rust changes. This establishes provenance and
source-port checks, independently of compilation and runtime outcomes.

The unchanged two-case runtime replay, using `LRR_EMBEDDED_FACADE=1`, the fresh
CLI/digester and owning edition 2015, terminated one as a selected batch.
`generic-derived-type.rs` passed all 13 recorded runtime checks and printed
`10\n10\n`; `repr128.rs` failed in the executor with `LRR-E-OP-TYPE`,
`operator requires supported concrete primitive operand descriptors`. The
failed case has no passing runtime-check record. No new stock gain is claimed.

The lead separately authorized one additional primary unchanged runtime witness,
`tests/ui/associated-types/assoc-type-in-struct-constructor.rs`, to identify the
next reached integration gap. Exact reproduction command:

```sh
LRR_EMBEDDED_FACADE=1 /opt/homebrew/bin/python3.12 tools/rustc_checkout_runtime.py --lrr target/debug/lrr --source-digester target/debug/examples/lrr_source_digests --report-dir /private/tmp/lrr-v505-associated-source-runtime --timeout 240 --case tests/ui/associated-types/assoc-type-in-struct-constructor.rs
```

The harness terminated one; its actual CLI invocation was:

```sh
/Users/avi/Code/rust-jit/target/debug/lrr --edition 2015 run /Users/avi/Code/rust-jit/fixtures/upstream/rustc-checkout/tests/ui/associated-types/assoc-type-in-struct-constructor.rs --runtime-capture --capture-context 3f8c1f081cb7ad32e7f623cc95cbac6882a7f1641bced45a92686a9fbe660407
```

The CLI terminated 2 after 3.763s, without timeout/output truncation, with
executor/fault `LRR-E-UNSUPPORTED`: `constructor field type requires compound
source type binding`. The retained capture has 6,079 stdout bytes, SHA256
`fc9db2a2201d6a1bbf32ff89b9a8b7bdc3bf3a7712a0e4aa70c060f2b8136f2b`,
and zero stderr bytes. The imported source SHA256 is
`8b7183304d6ee634fb3484db7aa3a00ece816496df723a904754198475de58b3`.
Both runtime reports retain unchanged original/manifest/binary input guards,
fresh digester identity, `native_execution:false` and `guest_jit_allowed:false`.
The associated witness is a separate failed runtime observation, not a new
acceptance or gain. Its reached target is actual `S<I>::field`/`I::T`
instantiation under `foo::<u8>` through the common member/projection route;
the source-clause controls do not repair that executor demand.

## V505 completed controls and closing guard

All six exact selected bodies terminated zero, each with exactly one executed
test and unchanged source/HEAD/test-binary guards. World counts below come from
the completed logs, including the first row prefixed by the test harness.

| Selected control | Seconds | Actual world rows | Component observation |
| --- | ---: | ---: | --- |
| Streaming GAT parent instantiation | 30.449 | 12 | 2 parent/1 own clauses, source-owner rebasing, spans, pinned methods and no encoding-time query |
| Own bundle versus HRTB hierarchy | 27.759 | 12 | own ready/empty without parent recipes; explicitly demanded hierarchy typed pending |
| Associated explicit-own recipes | 114.809 | 48 | 108 actual aliases asserted; real parent/Self/spans and Sized ownership |
| Associated own versus HRTB parent | 27.878 | 12 | method own ready/empty; explicit parent fetch HigherRanked pending |
| repr128 production Q80 projection | 28.125 | 12 | 36 own clauses/12 projected subjects; actual PartialEq IDs/arguments and independent source-only withdrawal |
| Existing shared impl-clause encoder | 143.411 | 60 | 144 clauses, 120 impl observations, 120 trait requirements, 12 region/12 Type requirements, 12 relaxations and 12 HigherRanked pending rows |

The first four controls exercise Q78/Q80/Q81 source preparation, not an upstream
solver root or executable entry. Each world spans one of editions 2015/2018/
2021/2024 and original/user-emitted/wrapper-emitted syntax. Applicable standard
and procedural macro execution, runtime dictionaries, complete implicit/inferred/
item/full compiler clause families and full fixture acceptance remain held. The
new hierarchy's epoch-first revocation does not establish independent source-only
hierarchy withdrawal. No source preparation was added to installed execution;
the eight-mode source review above is bound to the exact V505 fingerprint and
these component terminals, with its unexecuted matrix entries retained.

After all builds, seven audits, separate upstream verification, six controls and
both runtime harness invocations were terminal, `source_manifest.py after-runtime`
terminated zero: all 8,700 inputs and HEAD matched before, zero changed sources,
and both test/CLI hashes matched after-build. The separately captured fresh
digester also matched after-build at
`6e3677fd0fed33b77a1a4de7972d9449b798d60a371215c9f213c046b508265e`.
Source/HEAD/test/CLI/digester ownership was explicitly returned only after those
closing guards. Raw reproducible outputs are in
`artifacts/validation-current-v505`; required selections/tools remain outside
that disposable directory. The associated runtime failure is a next reached
integration observation, not proof of a particular unresolved branch deeper in
the source-binding path. No additional tests or source repairs were performed.
