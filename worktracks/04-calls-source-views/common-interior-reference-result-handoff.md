# Common source-certified interior reference results

Pre-code bounded record, LOCAL-024/WT04 T014/T024/T032/M014/M017/M018;
October 15, 2026. Successor to immutable named-source packet SOURCE SHA
`cda242b98d0cbd1ef39c74d236c52ddbd99b7d397ed75675384bd83185ce61db`.
Its old source/evidence stays unchanged. No Cargo/native tests, agents, commits,
pushes or ledger/task closure by this worker; sole validator owns execution.

Invariant: a returned reference may name an actual source-selected projection of
an eligible borrowed input, provided declared lifetime linkage, actual caller
region, store session, source definition/schema/member and the complete live
ownership/target chain all agree. Same type/layout, same address endpoints or an
uncertified raw field ordinal cannot prove that relationship. Ordinary source,
retained/generated functions and macro wrappers must join this same mechanism.

Primary unchanged witness is already imported rustc
`tests/ui/regions/regions-infer-borrow-scope.rs`, commit
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, blob
`2f25d0010833b01fc533e3bd5eeefadf5892ec1b`, SHA-256
`b296c6d78fbf362d7ab196a4dd3418f8bc12f8a6c365bbfe5c0a5a75b6473979`.
Its original `x_coord(p:&Point)->&isize {return &p.x;}` is the minimal runtime-body
frontier. Keep the file/directives/licenses unchanged. Full stock main also needs
Box and standard assertion expansion/library execution; an external unchanged-body
probe cannot count as a stock full-main pass.

Actual route: exact ReferenceHeader source proof → common SignatureNames/
source_types and current Type receipt → actual caller reference capture → declared
result contract/GenericEnv → ordinary ParameterPlace/ProjectField. Reached member
selection uses existing engine/run_fields and SchemaNames::bind_named_field, with
opaque BoundNominalField. GuestStore::project_bound_field already validates exact
source nominal DefKey/schema/variant/field/primitive against the real place, but
then returns raw Projection::Field and loses the source witness. resolve_place
retains runtime EnclosingObject chains but no selected source projection proof.
ReferenceResultContract also incorrectly filters input lifetime candidates by
identical pointee type. Extend these shared mechanisms, not another evaluator or
a special x/Point rule.

Add an opaque fixed-size source-certified nominal field projection variant to the
ordinary Projection enum, minted only from the existing selected member witness
after its checks. Place's existing projection list carries it across operation
phases without new Frame state. Raw Field/Tuple/Index remain ordinary addresses
but carry no new nominal source authority. resolve_place records sparse certified
chain positions alongside actual EnclosingObject links and preserves those source
receipts through deref/reborrow in ReferenceRecord. Both receipt and owning chain
remain bounded/accounted; no raw pointer, fabricated ticket or runtime source
inference. Existing SourceType proof supplies the expected origin and result
TypeKeys, separately from runtime correspondence.

Result lifetime candidate selection must follow the declared input lifetime,
elision count and source access, independently of pointee equality. It still
requires compatible actual input regions. Exact identity returns retain their
existing full target equality checks. A derived-return API validates the actual
ticket/owner/record/session, the complete original input chain, the complete
returned chain and exact original prefix, then a source-certified suffix from the
original final cell. Each suffix member must match selected canonical definition,
schema, variant, ordinal and result primitive, not a field name or observed
layout. The current common member service binds primitive nominal fields only;
nested nominal/tuple/array source projection proofs remain explicit prerequisites
where missing. Do not replace generic source member selection with a fixture list.

Return admission occurs before callee teardown/publication, with existing cleanup
continuation and final ticket recheck retained. A callee-local equal-typed target,
stale/reparented intermediate, raw unproved field ordinal, wrong source schema,
foreign store or changed source/epoch must refuse without publishing a caller
result. Returning a selected field may change pointee type but does not authorize
arbitrary equal-typed sublocations.

Cold cost: unchanged pure signature/schema/member queries and source Type/current
receipt admission. Reached source field projection emits a fixed-size certificate;
place resolution/reborrow copies its bounded source steps alongside the existing
bounded owner chain. Runtime return checking is O(original + returned chain +
certified suffix), without parsing, source planning, namespace maps or layout-based
source inference. Projection enum storage grows and existing size_of-based place
accounting must be inspected; ReferenceRecord's new receipt allocation must use
checked combined bytes and existing arena quotas. No new DB cache or immutable
BodyIndex copy. Coordinate runtime/place/reference/cell symbols with pattern peer;
root owns BodyIndex/thunks/cache dependencies.

Required controls outside originals: unchanged x_coord body through ordinary VM
call/field projection/borrow/return/cleanup; same operation handwritten/user output/
wrapper/retained caller where shared member origin authority permits; wrong input,
wrong schema/member/variant, raw projection versus certified projection, altered
original chain/reparenting, wrong store/ticket, named/elided lifetime, failure before
cleanup/publication, and replay/suspension currentness. Prior unsupported authority
control becomes a positive unchanged-body probe only after actual implementation;
full stock-main remains a gap. Source-review all eight recurring modes with actual
symbols and exact fingerprint before handoff; MA001–MA008 remain separately named
held audit/removal obligations. Formatter/source review does not establish runtime
or architectural acceptance.


## Source-only implementation and separate review

Seven Rust leaves form this successor. `runtime/place.rs::{Projection::SourceField,
SourceFieldProjection,SourceProjectionStep,ResolvedPlace}` retain opaque fixed-size
selected source facts and sparse chain positions. `runtime/cell.rs::project_bound_field`
now emits the certificate only after its existing exact DefKey/schema/variant/member,
primitive and installed child-owner checks; its former raw-Field publication is
replaced. `resolve_place_for_use` preserves certified prefixes through dereference
and reborrow, appends actual checked certified edges, and checks combined owner-chain
plus source-step allocation bounds. `validate_reference_record` replays both real
ownership and each retained source member against its actual parent descriptor.
Raw Field/Tuple/Index addresses remain available for their existing operations but
cannot produce this new nominal source receipt.

`runtime/reference.rs::ReferenceRecord::{from_place,owned_bytes}` retain and charge
source steps through the existing reference arena. `GuestStore::{borrow,
reserve_place_borrow_result,borrow_ticket}` already charge `owned_bytes`; no new arena
or quota bypass was introduced. Place and peer Frame projection storage already
charge `size_of::<Projection>`; enum growth is covered. Reference arena capacity
charges `size_of::<ReferenceRecord>` independently. Temporary combined buffers are
checked against the existing 32 MiB bound before a lease is published. The new
record adds one Vec header plus only the actually selected sparse source steps;
ordinary primitive references have no allocated source-step buffer.

`runtime/cell_reference_return.rs::require_reference_projection_return` is a readonly
sibling to the unchanged exact-target `require_reference_return`. It requires the
actual source-certified origin TypeId, actual expected result TypeId, same store
session, original full current owner chain, actual ticket owner/kind/reference record,
exact original chain prefix and exactly one additional selected nominal primitive
member edge from the original final stamped cell. It validates source key/schema,
active variant/ordinal, real child ownership and primitive result identity. No raw
ordinal, endpoint equality or runtime layout establishes the source relationship.
Primitive field membership is the existing shared member service's current boundary;
multiple/new structural suffixes are not silently admitted.

`frontend/closed_reference_results.rs::ReferenceResultContract::capture` removes the
incorrect same-pointee filter from lifetime candidate selection. Declared named or
elided lifetime/access still govern candidates and actual input regions must agree.
`check_ticket` chooses the unchanged exact-target check for equal source pointees and
the new derived check with both opaque SourceType descriptor proofs otherwise.
Actual return source/access/region and current source/definition/Type proofs remain
in `admit_reference_return`; no public ArgumentType(TypeId) is treated as authority.
No engine continuation changes were needed: inspected `engine/run_return.rs` checks
before cleanup, during each cleanup chunk and immediately before caller publication;
all checks share the same `check_ticket`. `engine/run.rs::check_source_return`
revalidates the real callee and certified result packet. Source/epoch revision guards
remain conservative; unrelated-namespace optimization is separate retained debt.

Source-only controls outside originals now include the actual unchanged x_coord
body with cold untouched stock main, actual call/BodyStructure/ordinary opcode and
cleanup counters, named-lifetime selected-field return, and rejection of an equal
schema callee-local field. Store controls use SchemaNames against the unchanged
original Point declaration to obtain real opaque BoundNominalVariant/Field proofs:
selected projection return, reborrow, input-reference cleanup, raw ordinal denial,
equal-valued other input, wrong source input/output type/access, wrong source schema,
corrupt chain position, cross-store session, and original intermediate reparenting
with unchanged endpoint stamps. The latter obtains a fresh valid result chain after
adversarial reparenting and still refuses the stale original input authority.
Only the existing cfg(test) source-selection helper visibility was widened to its
parent module for reuse; no new production constructor exposes source certificates.

Separate source-aware eight-mode review of the final seven-leaf fingerprint:

1. No assertion/expression evaluator added. Inspected `capture/check_ticket`,
   `project_bound_field` and ordinary `run_return` call sites; actual Call,
   ProjectField, Borrow and Return remain the operation route. MA001/MA007/MA008
   alternate macro evaluators/index/verifier routes remain held elsewhere; this
   component does not claim their removal. Actual route counters are unexecuted.
2. No literal/break-only output route added. The unchanged witness includes an
   ordinary return and member borrow. No macro identity is consumed by any new
   symbol. MA004/MA005 existing output shortcuts remain held, unclosed.
3. Source member authority comes from the same SchemaNames::bind_named_field used
   by ordinary fields; no fixture names occur in production code. Expected input
   and output source types come from SourceType, not tag/layout inference. MA003
   numeric and MA006 method shortcuts are untouched and held.
4. Runtime receipt checking has no original/retained/generated branch. Existing
   `engine/run_fields::prepare_field` still needs actual retained/generated caller
   member-origin authority: it searches original source owners, calls
   require_original_hygiene/named_field_member and rejects absent owners. This
   prerequisite was reported to root; it must be repaired through the common
   source/member service before the derived operation's generated/wrapper matrix
   can pass. No generated exclusion was removed without proof or newly added.
5. No pattern engine/eager arm preparation added. Header lifetime/type facts do
   not demand the callee body; the ordinary reached field operation selects its
   member. Stock main remains cold in the probe. MA002 transactional pattern repair
   belongs to peer; all applicable full-pattern runtime evidence remains separate.
6. No parsing/query/planning was added to installed runtime operations or return
   cleanup. Cached ReferenceHeader and existing selected Type/member preparations
   remain shared. Runtime source-step validation is O(owner links + selected source
   steps + encoded canonical DefKey sizes); reborrow copies bounded vectors, and
   return checks also compare the original prefix and binary-search the selected
   suffix position. Existing reached member preparation costs in run_fields remain
   separate debt. No speed claim or representative measurement was made.
7. No namespace-map scan added to any new operation. Only relevant runtime owner,
   cell generation, actual reference record and source descriptor are examined.
   Existing source revision/currentness safety refusals and source catalogue
   preparation are retained, not weakened by this component.
8. No BodyIndex/parser clone or whole-file node search added. Runtime copies only
   bounded mutable address/lease provenance. Existing Type proof/ReferenceHeader
   Arc ownership stays unchanged; new buffers use checked arithmetic and arena
   accounting. Parent/peer immutable body cache/index changes are outside this
   seven-leaf review and require their own exact combined-source review.

Remaining acceptance gaps: all new Rust is **uncompiled and unexecuted**. The sole
validator must run `runtime::cell::reference_return::tests`,
`engine::run_return::reference_result_tests`, and existing reference/source-Type,
currentness/epoch and member projection regressions against a coherent combined
source freeze including peer Frame/Step source-storage fixes. The original fixture
hash is unchanged (243 bytes, SHA above); no test changed a licensed original.
The unchanged-body probe is not a whole-stock pass. Box, std macro/library expansion,
derived generated/wrapper/standard-macro operation matrix, suspension/source/epoch
replay of the derived operation, multi-step/tuple/array/nominal-valued member proof,
full cleanup/panic matrix and milestone remain unexecuted or prerequisite-held.
No feature, architecture audit finding, task, runtime gate or stock fixture is closed.
Formatter/check terminals below establish syntax/format/diff hygiene only.

Final source fingerprint: `e5936bfa47a93dd16892981515a833fa4898a2fff3e3a49cfb27b69d4bb14f2f`. Encoding: SHA256 of sorted
`sha256 + two spaces + relative path + LF` rows:

```text
0721dcf6ac172455f62be4190085d9069549e9341fa12d8eef76d604a4e9bd09  crates/lrr/src/engine/run_reference_result_tests.rs
0a53994384ff43830250a1e91c74c4e750563b0b9e1f3ab837d4eda765c3ad11  crates/lrr/src/frontend/closed_reference_results.rs
46b3bbc01fe88425d1aafee02ecd776a37dfc077d64dd1f7af52a785ab62cd65  crates/lrr/src/runtime/cell.rs
f5af4f20ade358235ac180c9094db13c803f2e3654d203eb302e9a9c57c2f36a  crates/lrr/src/runtime/cell_reference_return.rs
5de9828bc7d117869045673e8745c36fc99a26af911722877d671323f708653a  crates/lrr/src/runtime/cell_reference_return_tests.rs
65cc30cb0daf81a817a2772c5109e1aef0c0a45455e32a749eaa2f3ca5307acc  crates/lrr/src/runtime/place.rs
39a71cfd81d29feefe20d4560fe39bd2057489a298d1444829dbb70f985e2a3a  crates/lrr/src/runtime/reference.rs
```

Final rustfmt --edition 2024 --check over these seven leaves and targeted git diff
--check both exited 0. Source review only; no Rust/Cargo/native command ran.
The source is frozen for parent copy at this handoff; later edits require a successor.
