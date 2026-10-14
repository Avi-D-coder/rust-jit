# M013: procedural discovery in captured physical modules

Revision 4 Sections 50.2/50.5/50.6 and fixed Section 57 milestone C.
macOS ARM64 Rust 1.99.0 first. M013 and release acceptance remain incomplete.

Invariant: an adopted physical module uses its consumer's prepared procedural
catalog and MacroRun session. Assembly retains the actual captured source, scope,
arguments and span callbacks. Neither module staging nor name lookup grants native
execution authority, namespace completeness or permission to publish a seal.

Staged modules now retain the owning procedural catalog. Discovery suspends with
an owned function-like selection and binds its exact request to the module job.
Only a matching coordinator-adopted output enters module integration. Ordinary
admission and pure expansion refuse native output/execution. MacroRun's existing
shared budget, input checks and session ownership cover the module request.

Macro route construction binds a captured module's access to its existing typed
namespace declaration. This exposes candidate paths through a staged physical
module while preserving unresolved frontiers and deferred body owners. Completed
expansions refresh module routes before namespace completion so their new imports
and macros participate in the fixed point. Shared completed routes are copied only
after checked metadata bounds; copies cannot mutate their retained originals.

Module readiness checks known procedural ambiguity even while import jobs remain
pending. Unresolved candidates remain pending. Final readiness re-resolves the
executed selections against completed membership and checks exact native identity
and epoch. Final MacroRun input verification and atomic publication remain required.

The positive actual-server control executes one original procedural invocation,
captures a generated physical module and its ordinary grandchild, then executes a
second procedural invocation in that grandchild through the same consumer session.
A generated declarative call and value import settle before both modules complete
and the seal publishes. Three outputs share one budget; unadopted output, direct
publication, another run's certificate and later epoch reuse refuse.

The negative actual-server control emits a conflicting import of a real compiled
function-like macro. It requires LRR-E-MACRO-AMBIGUOUS after two native invocations
and refuses seal/publication. The corrected root-source control requires that same
specific diagnostic after one invocation. See the
[evidence correction](errata/m013-procedural-ambiguity-evidence.md).

Initial failures are preserved: a non-cloneable authority in test setup, missing
module access facts, a value-namespace assertion for a type-namespace module,
shared completed-route ownership and the incorrect procedural export name in the
conflict fixture. The first milestone gate also caught raw/Unicode module-name
regressions; declaration access now uses the existing canonical-name helper. An
over-broad scripted edit was restored from recorded source literals and verified
against the historical names.rs SHA before the corrected gate. Validation results
belong to the checkpoint, not this document.

Attribute/derive transformation discovery, nested generated procedural requests,
full cross-unit/world/edition and Cargo matrices, complete CLI/service execution,
M014/M015/M016, broader language/traits/JIT/native runtime, security/performance,
release and Linux acceptance remain mandatory. No full M013 or release claim is
made by these controls.
