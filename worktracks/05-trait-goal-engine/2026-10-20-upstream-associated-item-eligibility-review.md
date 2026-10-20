# M020 associated-item eligibility — separate source review

Baseline5e3e92d08a2c41e174d146d0f7dffda3c2dd48c5; exact V487 fingerprint
bd1c9a0f9ec1e31835c686e4524d7e7d927b799122abd56db5607b45b0e5347e.
This is a held compiler-client component. No complete source-fact publisher,
specialization graph/leaf producer or actual shared root is delivered here.

The registered generator selects the entire pinned rustc
SolverDelegate::fetch_eligible_assoc_item method from the original delegate
file with SHA256ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095.
It mechanically adapts lifetime/IR/definition names, redirects the graph query
to immutable prepared leaf data and maps the leaf's item ID to the portable
record. All compiler final/default, typing-mode, deep-resolution,
further-specializable, source error and erased-result branches remain. The
old eligible_assoc_item function pointer and its dispatch are removed.
AssociatedLeaf::is_final is the passive finalizing-node Option accessor from
the compiler's LeafDef contract, not another selection algorithm.

PreparedFacts now requires explicit associated member and leaf outcome maps.
Publication checks recorded trait member coverage, definition/world IDs,
recorded parents/names, trait-impl container roles and the prepared ancestry
segment. These are structural typed-data checks: they are NOT evidence that
the missing source specialization producer computed a correct graph, complete
member inventory, source-parent receipt or diagnosed source error. That actual
producer/provenance integration remains mandatory. Missing leaves cause
preparation failure before ready; the callback has no missing-to-NotFound or
missing-to-NoSolution branch. Its expect is an internal ready-contract check,
not a semantic negative answer. No stub or fake complete provider is supplied.

Eight recurring modes (distinct from MA audit numbering):
1/2. No assertion or macro expression evaluator, alternate grammar, synthetic
     body or literal/break-only generated route. Changed production sites are
     the actual upstream SolverDelegate, its immutable fact reader/publication
     and the pinned generator.
3. No numeric inference, method-name or fixture-context rule. The complete
   compiler policy consumes the goal TraitRef and actual typing mode; legacy
   source inference is unchanged and remains migration debt.
4. No original-only branch or generated-source refusal. Portable source IDs,
   world membership and recorded parent relationships work across admitted
   source units; no same-file requirement was introduced. Actual generated
   membership/provenance acceptance requires the missing producer/matrix.
5. No pattern engine or unselected body/arm preparation. These are declaration
   membership/graph facts, not executable plans. No borrow/static checker or
   compiler-only impl-constrained enforcement was copied into runtime.
6. The reader does a relevant BTreeMap lookup; the policy uses pinned inference
   resolution/type flags. No parsing, queries, IO, macro/VM execution or recipe
   reconstruction occurs in the callback. Publication checks run cold before
   shared solver entry. Actual cold preparation/caching still needs integration.
7. No per-operation full namespace scan or unrelated invalidation is added.
   New map validation is within the existing source fact publication boundary,
   not dispatch or installed-recipe guards. Hot lookup is O(log prepared leaves).
8. No BodyIndex/parser copies or whole-file searches are added. Records retain
   portable source identities and existing metadata lifetimes. No new resource
   cap/accounting, unsafe, frontend, backend or query framework is introduced.

V487 both macOS1.99 builds and the registered inference-port audit terminate0.
Both selected existing source/context/inference controls pass across their
own edition/fixture scope. They explicitly keep root readiness rejected; they
do not execute the new eligibility policy or its new publication checks.
The first stock selector attempt exits2 before running because a path was
wrong; corrected unchanged repr128 still fails28/27VMattempt/outcomes at
generic operator typing, and generic-derived-type passes199matched operations
with expected10/newline/10/newline output. That runtime still uses the legacy
interpreter resolution path, not this staged client. Closing source/HEAD/test/
CLI guards pass. No new stock pass, root invocation, full fixture, task or
milestone acceptance is claimed.

Remaining clients: well-formed goals, constant evaluation and transmutability,
plus reflective field metadata. Required source graph/member/leaf/error receipt
production, full source/ADT/ParamEnv facts, genuine root calls and answer/member/
VM integration, invalidation and the complete negative/origin matrices remain
open. Do not count compile/audit success as executed eligibility branches.
