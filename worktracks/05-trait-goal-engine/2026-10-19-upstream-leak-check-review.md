# M020 actual leak checker — separate solo source review

Baselinea62de2ccd1e2170725adea44760eab5700280712; root/LOCAL-049/WT05-02.
Final candidate V349 reviewed fingerprint:
`70761d76dcd74bd5dd99a4b7434913eccb9084b2a322fdfdd7df51b9f571918e`.
Validation terminals remain separate until observed. No full feature acceptance.

## Actual algorithms, primitive boundary and remaining joins

The complete pinned compiler leak_check.rs is mechanically projected into the
one region collector. leak_check -> MiniGraph::new/iterate_region_constraints
-> actual VecGraph/Sccs -> LeakCheck::assign_placeholder_values/
propagate_scc_value is the compiler route. No homemade graph, union/cycle engine,
subregion test or canonicalizer. Current compiler equality expansion and visibility
are consumed directly. Snapshot-only selection uses the existing combined undo
mark/indexed constraint data, not a full-history scan or new snapshot owner.
Actual TypeError::RegionsInsufficientlyPolymorphic returns are preserved;
NoSolution/all-success/empty-region shims are not introduced or imported.

The graph module/traits/reference/reversed/iterate/SCC/VecGraph plus test closure
are complete selected upstream files. Binding excludes only unrelated dominators
and linked_graph modules. Import paths project to the existing adapter and same
rustc_hash aliases. The only algorithm-file control adaptation is the one unstable
debug_assert_matches -> debug_assert(matches) invariant spelling. Its predicate
and debug-only evaluation remain; host internal panic formatting is not exact
compiler diagnostic compatibility. No algorithm branch or test expectation changed.

All23upstream graph #[test] bodies are retained through explicitly adapted stable
packaging. One nightly extern test/benchmark is cfg-excluded and not executed or
claimed. Three empty &[usize] expectations get explicit types because serde_json
adds another PartialEq<usize> implementation in LRR. Original files remain exact
Git inputs. These are adapted compiler host components, not byte-identical full
modules or interpreter stock runtime passes, and no benchmark result follows.

newtype_index! emits unsafe functions. V348 only built; it is not the accepted
source. Final V349 imports dependency-owned LeakCheckNode/Scc from the shared
library, which compiles the exact compiler declarations with its supported
#[gate_rustc_only] option and public module spelling. Full original leak_check.rs
is retained. Constructor bounds, ordering, distinct roles and debug names remain
upstream. No local unsafe body or local index implementation is added. The
existing vendor projection registry now includes these primitives under explicit
stable-region-assumptions-v2-leak-indexes. Five compiled Rust files differ from
originals under registered projections; all retained originals are unchanged.
This is upstream primitive library reuse, not a local unsafe exception or relocation
of locally authored unsafe code. No RUSTC_BOOTSTRAP/nightly workaround is used.

The checker invariant branch needs Verify origin. Actual compiler Verify record,
add_verify and verify_generic_bound are projected into the same collector with
real incoming source Span as the existing origin adapter. Empty AllBounds remains
the original no-op. Nonempty Verify insertion records the actual origin and undo
index. The forbidden-Verify-during-HRTB panic retains its origin and failure; that
branch is source-reviewed but intentionally unexecuted by valid host controls.
It is not replaced by success. Compatible member/type-outlives origin migration,
full compiler cause variants and static diagnostics remain incomplete.

Lifetime/namespace projection accounts for RA collector parameter order and its
lifetime-free CombinedSnapshot. RelateResult/TypeError are actual rustc_type_ir
families; placeholder Regions and bounds retain actual Cx. Source factories grant
no ready token merely from running the checker. There is no InferCtxtLike/
SolverDelegate or owner-wrapper implementation using absent configuration flags.

## Eight recurring failure modes (separate from MA audit IDs)

1. No assertion/expression evaluator: Q69/Q70 -> source_terms::Encoder -> actual
   Cx/InferenceContext -> one collector/checker. VM/lowering/assertions untouched.
2. No literal/break generated engine: original/emit/wrapper source contexts use
   identical IR/checker code; no macro identity/grammar/source-origin branch.
3. No context-specific member/numeric inference: no dispatch/defaulting/goal
   algorithm change. Genuine leak errors preserve compiler term identities.
4. No generated-only refusal or fake facts: existing source/edition/epoch/current
   dependency guards remain; ready() still refuses incomplete source facts.
5. No duplicate patterns/eager bodies: no AST/body/thunk/pattern plan preparation.
   Region controls are explicit host IR, not a replacement guest language suite.
6. No repeated planning: only retained region data/undo events feed the compiler
   graph. No query/IO/macro execution/source parsing/installed plan reconstruction.
   Cold graph/SCC costs are upstream; no scaling/speed claim from source review.
7. No namespace census: graph nodes are actual supplied Regions; no global source
   namespace or candidate world construction/comparison. Snapshot scope remains
   relevant to the demand and excludes preexisting unrelated region evidence.
8. No BodyIndex clones/full-file search: no frames/bodies are touched. Graph and
   per-SCC arrays are the upstream transient data, not immutable source copies.

Separate MA004/MA007/MA008 closure and full source/runtime/macro authority/cleanup/
suspension/currentness matrices remain open. No static borrow/body/type/privacy
checker, resource accounting/limit, sandbox/native runtime path or early JIT.
The leak checker is for demanded trait/binder applicability, not inspecting all
unentered bodies. Source function binder classification, named/nested/full opaque
matrices, complete configuration/InferCtxtLike/generalizer/relations/canonical
responses/source facts/IDs/ParamEnv/lang-items, real delegate/root and selected
member/dictionary/ordinary VM migration remain open. Legacy custom consumers
remain held. Zero new stock runtime gains or refreshed suite total; no M020/
milestone/interpreter/JIT/Linux/performance/diagnostic/benchmark gate closure.

V347 compile failure (26errors: stable macro option and host empty-slice type
ambiguity) and V348 build-only source remain attached to their actual fingerprints.
Final V349 requires fresh source/binary guards and actual control terminals before
acceptance. Earlier build-only sources are not promoted by later success.

E-R5-006 index bound citation: pinned rustc_index_macros/src/newtype.rs parses the
same optional max and emits MAX/from_usize/from_u32 with their bound assertions
(lines213–243); both declarations leave max exactly as pinned leak_check.rs.
The stable-client flag gates compiler-only representation/traits, not the max or
checks. These are the compiler's own index representation limits, not new LRR
memory/node/work/fuel caps. All graph/test/checker sources locally contain no
explicit unsafe and the local checker no longer invokes newtype_index itself.

Final V349 library38522, CLI53326, provenance44703 and controls69970 all terminal exit0.26/26controls pass (23adapted upstream graph cases +3source context controls); source/HEAD/test/CLI guards match.140Types/136TraitRefs/48worlds and20generic worlds/48rows/36consts/24early regions observed. V347 library1327 exits101; V348 library36325 exits0 build-only without end guards or controls, unpromoted after unsafe primitive ownership repair. Default index max0xFFFF_FF00 is pinned rustc_index_macros/src/newtype.rs:98; same constructors/checks at223–243. No source changed after final review/validation. Actual trait root/runtime/milestone/JIT/full gates remain open. Exact commands, hashes, errors and terminals: worktracks/evidence/2026-10-19-v347-v349-upstream-leak-check.json.

Post-validation staged diff check found one trailing separator LF at EOF in the extracted index projection. Normalized only that LF, with asserted new==old[:-1] after closing } and no other compiled-source changes. All code tokens/positions unchanged; generator/inventory reproduce normalized bytes and final upstream audit passes. No runtime rerun for whitespace. V349 test/guard evidence stays attached to70761d76dcd74bd5dd99a4b7434913eccb9084b2a322fdfdd7df51b9f571918e; final normalized source fingerprint f1451d4d2f5c4479781ce50ebf21bcd79efbd88626ddc322b7b19b56bf72d2c5. No semantic/runtime conclusion needs rechecking from this EOF-only edit; exact source-byte freshness differs and is explicitly recorded. Full gates remain open.
