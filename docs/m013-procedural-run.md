# M013: procedural discovery owned by MacroRun

Revision 4 Sections 50.2/50.5/50.6, fixed Section 57 milestone C.
macOS ARM64 Rust 1.99.0 first. M013 and release acceptance remain incomplete.

Invariant: source discovery must use the actual MacroRun expansion ledger and
consumer sessions. A procedural seal may publish only immediately after that run
rechecks its recorded compile inputs and prepared files. Pure queries cannot start
coordinator work, and retained input certificates cannot authorize later publication.

MacroRun now opens or reuses its per-consumer session when requesting a prepared
catalog or assembling an owned source selection. Macro authority is checked before
session startup. Bound source requests must match the discovery's pending selection
before execution. Native execution verifies recorded inputs before and after work,
charges the shared ledger, adopts the exact output and integrates it into discovery.
Consumer sessions and retained output receipts have explicit count limits.

Final input certificates bind the graph, frozen request context, actual session
catalog fingerprints and adopted procedural output descriptors with a new explicit
stable-encoding domain. Publication checks that every procedural catalog/output in
discovery belongs to that verified run. The publication entry point rechecks actual
files, closes sessions and coordinators, validates the certificate and publishes the
complete discovery atomically. Direct procedural publication refuses, including
republication after a successful run. Declarative-only publication retains its path.

Actual matched-server controls exercise an original procedural invocation generating
a declarative macro and import, late macro-import ambiguity, and an actual recorded
source edit after expansion but before publication. They also check denied authority,
pure execution refusal, unadopted/substituted output refusal, independent-ledger
refusal before native execution, rejection of another empty run's certificate,
shared accounting, run closure and later epoch invalidation. A query-provider
control uses an invalid input plan to prove the coordinator guard precedes capture. Validation results belong to the checkpoint;
this document alone does not claim that a command passed.

Attribute/derive and nested generated procedural selection, cross-unit/world/edition
matrices, complete CLI/service orchestration, M014/M015/M016, broader language and
traits, production JIT/native runtime, security/performance/release and Linux
acceptance remain mandatory. These controls do not complete M013 or the release.
