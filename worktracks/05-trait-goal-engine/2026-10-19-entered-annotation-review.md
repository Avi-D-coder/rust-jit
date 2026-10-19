# Entered annotation completion — source review, gates HELD

Solo /root; baseline7da4e0dbfaad6f9f59f98be0083945f8bb85d2dd.
Final V306 source fingerprint:
`a26fe457cdcdd9585583b0a1b4c3e65dff4e4b04bfcd6ead5fa7d47c7be95a8a`.
This review establishes inspected routes; terminals and unexecuted matrices stay
separate. MA007/MA008 are the audit's preparation/currentness findings, not the
same numbers as recurring failure modes7/8 below.

1. **Separate assertion/expression evaluator:** no new expression semantics.
   `entered_declaration_thunk::thunk` reads an actual source plan and canonical
   metadata, obtains an opaque `EnteredDeclarationAdmission`, and calls
   `initial_thunk_with_declaration` -> `initial_thunk_admission` -> the existing
   `initial_declaration_thunk`. The ordinary verifier and `Op::Declaration` are
   unchanged. The new admission can name only an actual OpaqueItem at its exact
   indexed body/node/parse; it cannot admit an attributed expression or statement.
   There is no macro-name/trait-method/fixture-spelling production dispatch.
2. **Literal/break-only output:** source matching uses the actual initial
   `SourceAssemblySeed` and `SourceOccurrences::Original` header range. Raw
   SourceBlob coordinates are never normalized coordinates. Copied diagnostic
   origins do not become structural originals. Initial original declaration
   ancestry reuses `ParsedFile::declaration_scope_plan` and compares canonical
   IDs/header semantics before edits; current rows follow actual structural edits.
   Generated output names use the existing completed-fragment header projection.
3. **Context-only methods/inference:** retirement and name/pending insertion now
   converge at `DeclarationNameTables::prepare`/`extend_annotation_names` and
   `LocalTypeNames`; cold and entered clients use these same selectors. The actual
   body-local member index receives ordinary resolved metadata, not a method
   whitelist or new solver. Actual typed upstream solver invocation remains absent.
4. **Generated-source exclusion:** the new original receipt is deliberately
   bound to a physical original group; generated/copied occurrences cannot claim
   that receipt. Generated annotations still need their own genuine completion
   join. No new original-only runtime rule is added; existing unsupported paths
   remain explicit. Full generated local annotations, named outputs, nested
   replacing attributes/imports/descendant scopes, wrapper/standard-macro matrices
   remain incomplete and are not inferred from an original-only positive case.
5. **Duplicated patterns/eager arms:** pattern algorithms and static checking
   are untouched. Original annotation lookup filters known original body owners
   before requesting normalized declaration facts; it never lowers a method body.
   Body metadata is declaration-only. The new source control checks the actual
   normal Op::Declaration and that its thunk demand prepares no other body.
   Generic/unresolved local-header behavior and the full runtime matrix remain work.
6. **Repeated planning (MA007):** the initial normalized parser and Outline are
   retained once per SourceAssemblySeed; each initial local declaration projection
   is cached by canonical original definition. No per-phase whole-file outline
   reconstruction. BodyEnvironment retains one PreparedBodyContext Arc and its
   authenticated completion projection; SourceLocalPlan provider reuses it.
   Repeat publication checks current opaque source/seal inputs before preparation.
   Per-declaration admission selects a retained BTreeMap location and direct
   canonical row, not a fresh source plan. Installed Op::Declaration performs no
   source planning. No speed/scaling measurement or claim is made.
7. **World scans/currentness (MA008):** SourceDiscovery's coordinator factory
   authenticates receipts against actual completed original groups and exact
   adopted fragment custody; both cold and completed runtime environments consume
   that factory. New recipe preparation observes BodyEnvironment/SourceLocalPlan
   and DefinitionHeader/DefinitionSource/DeclarationContext. Epoch and replaced
   source guards remain in the actual query/input path. No installed full-world
   namespace comparison is added. Generated publication now reuses only matching
   actual units AND boundary outcomes/fingerprints after member-index creation;
   changed or first late generations retain refusal. Negative boundary diagnostics
   compare code/message; this is never promoted into a positive boundary receipt.
8. **Immutable body copies/node searches:** all body/index/context/name facts
   are shared through Arc. New ancestry traversal clones small ItemOutline metadata,
   not BodyIndex. Initial declaration matching is cold and cached; normal node
   admission uses an indexed location/direct row. No whole parsed-file search is
   added to dispatch or installed execution. Legacy accounting/retention systems
   were not extended; no new budgets, caps or sandbox/static-check rules.

Exact metadata publication is also shared: `DeclarationAnnotationCompletion`
selects an original or structurally retargeted source proof, then the existing
`publish_declaration_metadata`/`publish_declaration_row_metadata` checks and publishes
one canonical row. Annotated declarations without a real completion remain
NeedsProducer. Reached local calls use that same checked kernel. The former
unconditional attribute refusal remains for unsupported syntax; actual prepared
declarations carry their proof into ordinary lowering instead of catching errors.

Failed attempts retained: V301 three mechanical compile errors; V302 reached
"completed member index was demanded before generated publication"; V303 reaches
the VM but fails the generic statement/expression attribute guard; V304 has one
QueryPoll<Option<Provided>> compile mismatch. V305 both builds and2/2controls pass,
including the unchanged whole main in all four editions and structural/header/
node/parser/epoch/generation refusals. V306 adds the retained normalized Outline
and selected-owner preparation filter; results must be collected against its own
source, not relabeled from V305. An accidental V301 after-runtime capture failed
because no build manifest existed; no tests executed in that attempt. Correct
source-only after-build evidence was subsequently recorded.

This is not closure of E-R5-005, M019/M020, full interpreter or JIT gates. The
complete selected generated impl self-Type control still requires eight local
origin/insertion joins, and arbitrary generated annotation/output/alias/import/
member/facts/inference/delegate matrices remain unexecuted or incomplete. Preserve
all actual failures and require complete applicable upstream runtime/correctness
acceptance before guest JIT.

## V306 observations and final ownership correction

V306 both builds pass; selected runner87355 exits1:12/16 controls pass. Whole-main,
new receipt/generation/epoch controls, cold original/Foo heads, Copy12origin worlds,
record execution, current source replacement and plain/user/wrapper local calls
pass. Full selected head control remains24/32 (eight generated self Types pending).
Both selected structural-occurrence controls fail during their existing request
setup ("actual source occurrence request incomplete"), before structural copying
is checked; these are gaps, with no measured parent attribution. Do not cite those
controls as a passing copied-origin proof. Source inspection of the map/matcher
is separate from this failed end-to-end evidence.

The older body-environment retirement control also fails: final retained roots
remain2924128bytes instead of zero. Actual owning edges explain this: the new
BodyEnvironment PreparedBodyContext Arc was omitted from the existing ownership
DAG, so an internal owner looked external. V307 adds that real reference edge and
preserves raw-context external-owner detection, without adding byte accounting,
resource limits or budgets. Declaration receipts now retain immutable facts and
small source/body/scope metadata, rather than extra PreparedBodyContext Arcs.
This avoids additional context backreferences; direct canonical row and source
checks are preserved. A real Weak-context release assertion extends the annotation
control after revocation/quiescent cleanup. These are ownership/cleanup changes,
not a raised/disabled resource limit or a relaxed test expectation.

Final V307 fingerprint:
`bdc8b11b01fe52217ca90f2f9acc5937ab93c54418fe950201e6c721b535b5af`.
The eight-mode source review was rechecked for these deltas: same existing header/
namespace/lowering route; direct immutable facts replace context ownership in the
receipt; product_retention counts actual physical reference edges, not allocations
or a new budget; wrong source/body/header/node/epoch guards remain. No new parser,
evaluator, namespace scan or BodyIndex copy. Final targeted controls and runtime
must be collected under this fingerprint; V306's12/16 and24/32 retain a26fe457
source, not relabeled as V307 broad acceptance.

Actual V306 CLI85708 exits0: unchanged Drop and coherence-impl-in-fn mains complete
all four editions (141 and4 operations respectively). This establishes one new
distinct stock runtime gain relative to the recorded V299 local-derive failure,
not eight new stock tests, a refreshed suite total, full-fixture/diagnostic/tooling
acceptance or macro/trait completion. Source, HEAD and CLI guards match. The source
control also confirms normal Op::Declaration and no body preparation beyond the
selected main during its thunk query; unused eq/ne bodies remain unexecuted.

## Final V307 collected evidence

Both actual Cargo build-finished records report success; library session62751 exits0. The CLI build and five-control runner handles were lost at context truncation, so their outer shell exits are not invented. Each of the five child controls exits0 with unchanged source/test-binary guards. This includes the old physical ownership retirement control and the added Weak-context release, plus ordinary local calls and all12Copy origin/edition worlds. Direct CLI session53565 exits0, all8edition worlds complete across the two unchanged roots. Source, HEAD, test binary and CLI guards match final fingerprintbdc8b11b01fe52217ca90f2f9acc5937ab93c54418fe950201e6c721b535b5af. V306 broad12/16andhead24/32 remains historical, not a final broad rerun. Both copied-origin end-to-end controls remain request-setup failures/unverified; full generated local/name/output and source-backed solver matrices remain held. Bounded evidence retains the compile/probe/retirement failures and exact final commands. No source speed, full-fixture, task, interpreter/JIT or Linux acceptance.
