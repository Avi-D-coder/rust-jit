# M020 modern region collector — separate solo source review

Baseline6ced27f7d3897b281e5fa7e9b79d4f500b382ab3; root/LOCAL-049/WT05-02.
V346 reviewed source fingerprint:
`b9f1e3b777d15e47b0b0a08c5490631d0cb2ac2e60fcece25061b4288a8af5a8`.
Validation terminals are attached separately; this review is not acceptance.

The actual pinned compiler region_constraints/mod.rs is retained unchanged.
ConstraintKind/Constraint/iter_outlives plus add_constraint/make_eqregion/
make_subregion/combine_vars and shallow/try region resolution are complete
selected compiler bodies, registered by upstream_region_collector.py through
sync_upstream_inference.py. The old four-case enum and two-subregion equality
implementation are replaced, not wrapped. Old opportunistic_resolve_var and
probe_value collector entry points are removed; existing host callers and the
shared collector universe operation use canonical current names. The similarly
named underlying ena probe API is intentionally unchanged.

The compiler's visibility flag is the actual shared solve::VisibleForLeakCheck,
not a local boolean or substituted default. ConstraintKind::VarEqVar/VarEqReg/
RegEqReg preserve equality until actual iter_outlives requests its subregion
edges. Direct-subregion kinds retain compiler reflexive/static no-op rules.
Equality with static uses the compiler's one-way edge rules. Collection records
both Yes and No; it does not implement leak filtering or claim successful checks.

Source origin is the actual incoming Span retained alongside each constraint,
matching the current compiler Vec<(Constraint,SubregionOrigin)> layout through
an explicit Span data adapter. Full rustc diagnostic cause variants are outside
this slice and not claimed. Compiler invariant panic spelling keeps the incoming
source origin; no invariant becomes success. Existing RA per-variable spans are
retained. Its extra Known-value optional span remains None, matching the old
compatible client; compiler Known has no such field. No actual variable/constraint
source span is replaced by a dummy or guessed origin. SubregionOrigin is not
fabricated and a full source function/binder classifier is not implied.

Compiler combination creation and its explicit VisibleForLeakCheck::Yes policy
are reused. Existing compatible RA TwoRegions/CombineMapType receive compiler
Copy derives for these entirely copyable records. No changed LUB/GLB algorithm:
the suspicious pinned static LUB branch is deliberately preserved, no independent
upstream bug fix or broad lattice correctness is claimed. Compatible RA Verify/
member records remain; compiler origin transport for them and complete modern
collector migration remain held. Current unification/snapshot owner remains one.

V345 build failed with11mechanical errors: compiler-middle visibility reexport
versus the actual shared solve namespace, and omitted std::iter import. No guest
or host controls ran. The exact failed source and diagnostics are retained.
V346 changes those import spellings only, then rebuilds before any control claim.
No inference rule was adjusted to make a test pass.

## Eight recurring failure modes (separate from MA audit IDs)

1. No expression/assertion interpreter: source Q69/Q70 -> Encoder -> actual Cx ->
   InferenceContext/one collector remains the route. VM/lowering untouched.
2. No generated scalar/break engine: same typed collector for original, arbitrary
   user emit and wrapper worlds; no macro-name, grammar or origin-specific rule.
3. No context-only member/numeric inference: no new member choice/defaulting or
   goal algorithm. Actual compiler region kinds and mutators replace old client
   infrastructure; actual runtime trait selection remains unimplemented here.
4. No generated-only refusal: existing prepared identity/edition/epoch/source
   guards stay owners. Collection grants no ReadyProvider or full source facts.
5. No duplicate patterns/eager branches: no pattern engine, unselected arm or
   body preparation. The added cases are explicit host IR controls, not synthetic
   Rust guest tests or a substitute language suite.
6. No repeated planning: collector operations receive interned Regions and actual
   spans. No source IO/query/macro expansion/parser or installed plan rebuild.
   Costs are the pinned vector writes, union operations and explicit iterator;
   no speed or scaling claim is made.
7. No full-world namespace census: no catalog reconstruction/global map compare
   or unrelated invalidation. Currentness still belongs to prepared source facts.
8. No BodyIndex copies/full-file search: no bodies/frames are touched. Current
   combined undo records remove actual constraint/span tuples and undo unions;
   no second snapshot engine or deep immutable source clone.

Separate MA004/MA007/MA008 closure and broader runtime/origin/cleanup matrices
remain open. No new unsafe/resource limit/accounting/sandbox/static checking.
Actual upstream graph/SCC helper reuse and leak checker are still absent. The RA
unconditional leak-check success and empty response stubs remain excluded; this
slice does not introduce or promote a SolverDelegate. Full InferCtxtLike,
generalizer/relations/canonicalization, source facts/ParamEnv/IDs/lang-items,
source function binders/currentness, selected dictionaries/member/VM joins and
superseded custom consumer removal remain prerequisites. No actual root solve,
stock runtime gain, refreshed suite count, M020/milestone/interpreter/JIT/Linux/
performance/diagnostic acceptance follows from host matrices.

Final V346 library77919, CLI19454 and controls9572 terminal exit0;3/3controls pass, source/HEAD/test/CLI guards match.140Types/136TraitRefs/48worlds and20generic worlds/48rows/36consts/24early regions observed. V345 library85603 exits101 with11mechanical errors; no controls ran. Source stayed unchanged after final review/validation. All new region cases are supplemental host IR; no root/stock-runtime/gate acceptance. Exact commands, hashes, errors and terminals: worktracks/evidence/2026-10-19-v345-v346-upstream-region-collector.json.
