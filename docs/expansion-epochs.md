# Expansion epoch eligibility — M012 in progress

ExpansionOutputCache stores pure owned token facts only. MacroSession fulfilment
runs before cache validation, preserving exact-once session semantics and fresh
execution in a later epoch. Each returned certificate binds the complete demand
context; a separate semantic digest omits epoch alone and retains unit, tool,
original input, hygiene/context, environment, editions, grammar and output.

ExpansionEpoch is an explicitly published root input (stable value tag 23).
CurrentExpansionInput is a separate active-run query (stable query tag 21). A
procedural key requires the matching epoch input. An absent/mismatched epoch is a
request-context error, not a missing macro result or negative semantic lookup.
A matching epoch with no expansion record returns NeedInput. Ordinary query
stamps invalidate changes/removal, and derived publication rejects an epoch that
conflicts with the published active epoch. Historical ExpansionInput addresses
an exact immutable record without asserting current-run eligibility.

The actual external-file service regression now publishes epoch and adopts
through the active query. A standalone database regression checks missing epoch,
matching adoption, recorded epoch dependency, stale adoption refusal, renewed
expansion demand and immutable retained snapshots. Gate01 observes both tests.

Run orchestration must publish the frozen epoch before active queries and use
CurrentExpansionInput for current eligibility; migration of remaining callers is
pending. Historical reads are not authorization to reuse a procedural result.
Recorded source/build pre/post verification, multi-unit orchestration, generated
seal/guest assertions and complete MX025/MX026 acceptance remain pending. This
checkpoint does not claim M012 completion or hardened native macro containment.

RecordedMacroInputs now retains an explicit capability InputPlan plus immutable
preparation and before-capture witnesses. It reuses bounded preparation capture
and compiled-file hashing; build-output fingerprints are checked against the
recorded tree. Verified fulfilment checks before and after native expansion and
before returning a cache certificate. Input failure closes both session and
coordinator. The actual macro mutation regression observes one call and refuses
adoption/retry; the pre-invocation edit observes zero calls. The same guard can
be checked again before seal publication, whose integration remains pending.
Undeclared native reads stay non-hermetic. Build/tool/membership mutation matrices
and whole-run ownership are still required before M012 completion.

MacroRun now owns unit sessions and coordinators within one frozen revision/epoch.
It checks the published epoch and source revision, opens a matched authorized
session lazily for a manifest unit, verifies inputs around fulfilment and adopts
the output using CurrentExpansionInput. Errors close the entire run. Final
verify_for_seal checks the input witness again, returns its verification identity
and closes all sessions; this identity is not a declaration/catalog seal.
The actual single-unit counter regression shares an invocation across consumers,
observes a distinct invocation at 2, starts at 1 again in a later epoch, refuses
after closure and denies native invocation without macro authority even when
pure output facts remain cached. Actual multi-unit Cargo preparation, typed
catalog-seal integration, remaining entry-point migration and full acceptance
remain pending.

Actual multi-unit acceptance now uses one pinned Cargo root with a live library
dependency, both consuming the same prepared macro host. Compiler argv checks
prove neither invalid live body was compiled. A single MacroRun opens independent
unit sessions: each first invocation yields 1, repeated consumers reuse locally,
and a second distinct invocation yields 2 for each unit. Semantic identities
differ by unit even for equal output text. The rejected earlier two-root fixture
is retained; graph validation was not changed. Production graph integration still
belongs to M038. Remaining entry-point migration, typed seal verification and
mutation/semantic matrices remain pending before M012 acceptance.

Final input verification now returns VerifiedMacroRunInputs with private fields.
It has no public constructor or decoder. Its identity digest is reporting data,
not a substitute for the typed certificate. Validation checks the exact demand
context/preparation, database source revision and active epoch. Actual positive
certificates reject other contexts/preparations and epoch republication. A
source edit after an accepted expansion but before final verification issues no
certificate and closes the run; retries do not invoke again. Retained pure facts
remain available only after future fresh validation. This is not evidence of
catalog completeness; M013 must validate the certificate at its publication
boundary along with all required producer and membership facts.
