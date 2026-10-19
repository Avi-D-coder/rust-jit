# Generated local Type lookup — source review, gates held

Solo /root; baseline bebda6f57489aa48fc3ffd1caf3632c8472d168b.
Reviewed V310 source fingerprint:
`72430a386e49a7cf41d2ce0ccc7324d311c4fcc0fdf06bfd039d300ee8b7606d`.
This is a source review; validation outcomes must be attached separately.

1. No assertion/expression interpreter is added. `source_types::resolve_actual_type`
   calls `type_name_namespace::resolve_declaration_name_at`, whose existing local
   path calls `ColdDeclarationNames::lookup`. Original and generated callers meet
   at the same `ColdDeclarationNames::lookup_prefix` -> `DeclarationNameIndex::
   lookup_prefix` -> `LocalTypeNames::items`. Classification and runtime operations
   are unchanged. No macro/trait/method spelling selects behavior.
2. No literal/break-only output execution. `declaration_name_effects` uses the real
   completed fragment/index and retained output/header plan. Its new map binds
   exact record descriptor AND actual surviving declaration ID to insertion scope.
   `GeneratedHeaderPlan` active nominal/alias/function/impl/trait projections and
   replacement masks filter membership. No display-text parsing or copied-origin
   structural inference. MA004's common output requirement is preserved; this
   source slice does not close its complete expression/statement matrix.
3. No context-specific method or numeric policy. The generated occurrence's actual
   `GeneratedItems::project_source_spans`/`item_class` supplies the pinned item
   comparison class, and existing local names select the canonical definition.
   Original use sites retain their actual original comparison class. Source
   identities are not guessed from runtime layouts or name spelling.
4. The previously pending admitted generated local declaration can use the shared
   prefix. A completed output alone grants nothing outside its exact active
   declaration/insertion. Retained body-only generated edits, arbitrary generated
   annotation/module/parent/import cases still require their existing genuine
   joins; unsupported cases stay gaps. This does not authorize inventing an
   original local owner for an unrecognized output. MA007's catalog semantics
   requirement is not relaxed: the map transports source completion only.
5. No duplicate pattern handling or eager executable preparation. Only declared
   headers and their actual Type occurrences are read. The existing unchanged
  32-head control checks no BodyStructure/Thunk/PatternPlan/FunctionSignature
   demand. New revocation/restoration guards also inspect those actual trace keys.
   Whole-runtime regression does not prove unused trait method bodies execute.
6. No installed runtime parsing/planning. The new active-output map is produced
   once in the existing root/cfg-bound `OnceLock` name-effects projection, then
   joined once to immutable cold tables. The Type query observes the existing
   ParseFile memo and parser-owned occurrence proof; it does not invoke expansion,
   source IO, or manufacture a second parser/evaluator. This slice does not remove
   inherited repeated-planning costs elsewhere; no measured speed claim.
7. New queries observe only the exact declaration-name input, generated unit,
   canonical rows, actual ExpansionRecord, owning seal/source and epoch. No new
   namespace census or global dispatch comparison. Existing `require_current`
   remains the cold input's guard; this does not certify all inherited guard paths.
   Missing input stays pending and must invalidate a prior ready Type head; stale
   opaque handles cannot replace the coordinator's actual republication.
8. No BodyIndex deep copies or whole-file node searches. GeneratedSyntaxInput
   retains a deterministic canonical-ID-to-Outline-location map from its existing
   construction loop. Queries use that map and the common retained syntax locator.
   Completed effects retain small scalar IDs/scopes; no extra body/context ownership
   edges. New indexes are ordinary OS-owned data, with no byte/fuel/count budgets.
   MA008's macro-only index/verifier audit is separate: no such route is introduced,
   and its wider repair cannot be closed from these Type-header observations.

Superseded route removed for admitted cases: the unconditional original-ParseKey
mismatch return now calls the source-aware generated join, then the same prefix.
The generated_result_namespace module's legacy module-root-only route remains for
its distinct consumers; this change does not promote it or claim its full removal.

Cold costs: completed active range sets and ordered record/declaration maps are
built once; each generated name demand has ordered map lookups, actual origin
projection, memoized ParseFile access and existing lexical-prefix traversal.
No installed operation, normal dispatch or frame-entry work was added. No static
checker, resource accounting, sandbox authority or backend behavior was added.

V308 initial (before active-mask hardening) builds and passes32/32heads and16actual
generated occurrence controls. V309 failed to compile because SourceRange lacks
Ord; V310 uses scalar `(start,end)` keys. Both are retained as their actual source
cohorts. Cfg/replacement filtering is inspected source; dedicated complete replacing
attribute matrices remain unexecuted. No full M019/M020, true upstream solver
invocation, new distinct stock runtime gain, full fixture or interpreter/JIT gate
acceptance follows from this source slice. Prior structural-occurrence request
setup failures remain open, not inferred fixed by neighboring passing controls.

Review follow-up: use StableEncoder::map for the new output-declaration map so its sorted entries have an explicit count delimiter. This is a pending source delta until V310 users finish; do not mutate tested sources in flight or relabel V310 results after that change. Prior pending/name stream encoding is outside this slice.

Next concrete integration obligation from current source inspection: upstream/prepared.rs::PreparedProvider::publish requires implementations.len()==impl_declarations.len()==entire catalog headers.len(). A complete raw inventory is required, but entering a demanded goal must not require translating every unrelated impl into ready typed facts. Preserve inventory completeness while preparing only the demanded candidate/source dependency closure; do not satisfy this API using stub/empty fact rows or compile whole cold bodies. No producer or real solver invocation exists yet. This is the next source/inference bridge repair after this local Type checkpoint, not evidence of an implemented fix.

Final V311 source fingerprint1b2f509100f6d016b2bc63f72772154adc1c0e8dae7d9405adef7d2e6db43ba7: the only V310 code delta is StableEncoder::map replacing the new output loop, with an explicit entry count and deterministic ordered keys. All8source-review conclusions were rechecked for that delta; no lookup/lowering/runtime/ownership rule changed. Final library91485,CLI84919 and control57852 terminals all exit0; four affected controls pass with source/HEAD/both binary guards matching. Full selected32-head control still32/32 with16actual generated occurrence controls; real input/owner/header/epoch restoration, entered receipt/Weak retirement and unchanged main controls pass. V310 broader8/8including12Copy worlds remains historical pre-encoding source. No new distinct stock runtime gain or solver/task/interpreter/JIT closure. Cfg/replacement filtering is source-inspected, not an executed complete replacement matrix. Same-record validation may compare the retained wire bytes: O(record bytes) on cold Type preparation, with no new execution hot-path cost.
