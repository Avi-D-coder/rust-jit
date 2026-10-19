# M020 — actual compiler goal fast paths and type-property helpers

Solo root/LOCAL-049/WT05-02, baseline400b3dd2a0d3fc2c7119164780592f143f392252.
E-R5-014/spec53.0 requires real delegate behavior. Unchanged runtime witness stays
traits/multidispatch1.rs (latest V356 configured get ambiguity); new component
controls must not be called original-main passes or complete root integration.

Ordinary/generated source Q69/Q70 -> Encoder/actual Cx -> InferenceContext ->
complete pinned compute_goal_fast_path plus original stalled helpers/visitor,
known_no_opaque_types_in_storage, sizedness_fast_path, Ty trivial-sizedness/
copy-clone/WF/ty_vid and Term WF methods. Shared Const already supplies ct_vid/WF.
No method-name whitelist, fake SourceFacts/lang-item ID, no-op callback or constant
NoFastPath/TriviallyHolds result substitutes for those actual bodies. Add helpers
to existing owner; do not claim the full SolverDelegate trait is implemented.

Transport compiler LangItem Sized/MetaSized/Copy/Clone through the actual shared
SolverTraitLangItem and existing retained source service, not a name lookup.
Trait/dyn/ADT/ParamEnv-dependent branches require genuine complete prepared facts;
keep ReadyProvider closed until their source producer exists. Source controls can
exercise primitive/state branches and genuine source TraitRef with unresolved
Self (stall before metadata demand); metadata-dependent positive branches stay
explicitly unexecuted, not proven by synthetic providers or invented definitions.

Reuse exact licensed pinned sty.rs/util.rs/mod.rs/delegate.rs bodies with recorded
hashes. Mechanical IR ownership/lifetime/import/Span/argument-order/visitor stable
Result transport only. ThinVec uses already locked0.2.21, adding direct workspace
references without selecting another version/dependency. Existing source/member/
constraint route remains; no parser/query/IO/macro execution/whole-body checking.
No duplicate solver/generalizer/canonicalizer, opaque IDs or source-ready token.

The compiler tuple <=3 Copy/Clone optimization threshold is retained as a proof
fast path, never a guest-program cap or negative trait answer: false falls back
to full solving. Preserve NoFastPath vs TriviallyHolds vs TriviallyStalled and
real wake variables/subtype roots/opaque state/visibility. No return false may be
translated into NoSolution. Native/JIT/borrow/privacy/lint/resource/sandbox work
is out of scope. Cold recursive type-property work and actual visitor memoization
are retained, no global namespace census/BodyIndex clone/installed source plan.
No measured speed claim follows from reusing fast-path code.

Required evidence: exact source projection/vendor audits, frozen async macOS
1.99.0 library/CLI builds; unchanged upstream actual Type/Span/trait contexts
across editions/origins, positive primitive/ref/pointer/tuple property cases,
nontrivial fallback, unresolved/known numeric/type/const stalls and wake states,
source TraitRef unresolved Self/opaque-empty behavior, subtype/coerce root state,
region/type outlives flags and binder escape, real shared outcomes. Review all
8recurring modes against exact source fingerprint; preserve failing attempts.
Full source facts/trait/dyn/ADT/binder/opaque metadata and actual delegate/root/
canonical response/member/VM integration, full M020/interpreter/JIT gates remain
held. Metadata-only controls and code compilation are not runtime acceptance.
