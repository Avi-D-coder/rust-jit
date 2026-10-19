# M020 — pinned canonical-input instantiation client

Solo root/LOCAL-049/WT05-02; baseline e6f6983b2bbcaa5a0c44c07abb87ebd43ddf3a28.
E-R5-014/spec53.0 requires upstream canonicalization/instantiation/transactions,
not a bespoke canonical binder evaluator. Runtime witness: unchanged
traits/multidispatch1.rs (V356 configured runtime fails ambiguous get selection).
No ad hoc output/receiver-name selection may repair that witness.

Actual shared route: Q69/Q70 -> source Encoder -> retained Cx/PreparedProvider ->
InferenceContext/InferCtxtLike -> pinned compiler canonical/mod.rs methods and
canonical/instantiate.rs CanonicalExt/CanonicalInstantiator -> same shared
TypeFoldable/type/const/region/predicate/clause families and combined storage.
The upstream solver already contains its new canonicalizer; do not copy the
legacy compiler canonicalizer/query-response solver. Full SolverDelegate/root,
actual source facts/ParamEnv/member/VM join remain held.

Import exact licensed compiler canonical/mod.rs and instantiate.rs; deterministic
hash-pinned transport of full relevant bodies, including compiler clause/type
caches and highest-bound-variable visitor. Extension-macro syntax becomes an
ordinary trait declaration with unchanged method signatures/bodies. Compiler
builder ownership becomes the existing InferenceContext::new(actual Cx, actual
input typing mode); it retains the current source settings and one storage owner.
Do not add another context builder/solver, no-op callbacks or fake ready facts.

Compiler clause caches are immutable-input pure maps: highest canonical variable
per actual Clauses, then instantiated Clauses keyed by actual Clauses plus the
retained argument prefix. Own them beside existing context caches/arenas and
borrow them through StateRef; preserve short borrow lifetimes before recursive
folding. Do not place them in a global cache or scan source namespaces. Cached
handles never cross a source context/world. Empty/repeated canonical input and
later-universe/subtype roots retain actual compiler handling. Only mechanical
concrete type/span/list accessor/argument order/import/lock transport is allowed.
No RA legacy query response or expression/body inference is pulled in.

All editions and original/emit/wrapper source types reach the same constructor;
no macro identity or source-only exclusion. Source metadata stays cold/immutable;
canonical folding is cold solver work with retained upstream memoization, no
installed VM parser/query/plan reconstruction, BodyIndex clone or whole-file
search. No unsafe, sandbox/resource accounting/static checker/native runtime
compilation or JIT work. Original diagnostic variable-origin fidelity remains
explicitly partial as in V356.

Required evidence: exact originals/projection audit; frozen async macOS1.99.0 lib
and CLI builds; actual retained upstream source contexts exercising canonical
input/instantiation with real source Type/Span payload, mixed canonical kinds,
universe mapping, subtype roots, canonical vs ordinary binders, exact typed
replacement, predicates/clauses/cache reuse, zero-variable fast path, projection
and mismatched-length rejection. These are supplemental host route controls,
not original fixture main passes. Review all eight recurring modes against exact
source fingerprint. Keep M020/interpreter/JIT gates open until genuine roots and
runtime contracts pass; leave nonempty opaque/full fact/function binder/member
integration gaps explicit. Remove no sound currentness or identity guard.

V359 validates3/3controls at its recorded source, but the raw ordinary-Bound
transport control is not promoted as well-scoped nested-binder evidence. V360
changes only infer/tests.rs: the payload is a real Binder with one ordinary
bound type plus a canonical type inside its tuple. Check retained local binding,
canonical substitution within it and absence of escaping ordinary variables.
The production compiler algorithms/caches remain byte-identical. Run only the
affected source matrix after frozen final builds; unaffected generic/cache-scope
controls retain their V359 evidence, not a claimed all-three final-source run.
