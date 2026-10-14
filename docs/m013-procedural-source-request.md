# M013: source-owned function-like procedural request

Revision 4 Sections 50.2 and 57 milestone C, macOS ARM64 Rust 1.99.0 first.
M013 remains in progress; this is request materialization, not a complete worklist.

Invariant: an original active item-position function-like invocation is selected by
its immutable outline identity, resolved through the existing source macro index,
and matched to the exact prepared SDK export. Its argument subtree, call span,
consumer unit, scope and original source parent are preserved. Request construction
reads retained source/session facts and executes no helper or query side effect.

FunctionInvocationInput uses the existing pinned source token preparation, original
macro group selection and argument materialization. It never reconstructs arguments
from display text. Resolution uses the existing original-call resolver and prepared
procedural catalog, requiring the function-like kind and exact artifact/export.
The existing item-request assembler supplies bounded owned site/capture records and
validates the final key against the frozen MacroSession. The caller source capture
is assembled directly from the owned selected snapshot; callers cannot replace it
with supplied source text. Separate definition/mixed site records remain explicit.

One actual native acceptance control prepares the synthetic proc tool only, builds
source/import routes from its retained catalog, selects an alias invocation, and
observes zero invocations during assembly. It rejects wrong export or scope,
stale epoch, absent namespace and unknown source selection, excludes cfg-disabled
invocations, and distinguishes a later invocation's real call span. The authorized
coordinator then invokes the matched server once, observes only the selected
identity-macro function tokens, and replays without another execution. Owned source,
output token wire and native request report are retained as evidence.

The remaining bridge must schedule these requests in SourceDiscovery, merge prepared
facets and dependencies with current routes, revalidate selection after generated
membership changes, and integrate the exact returned record before sealing. Full
MacroRun input verification, shared accounting, all procedural forms, generated calls
and modules, CLI/service integration and M014/M015/M016 remain required. No namespace
seal or full MX027/MX028, JIT, security, release or Linux acceptance is claimed here.

The first milestone attempt is preserved as a failure: the existing four-second
MacroRun cumulative fixture timed out during preparation under concurrent load,
with zero invocations. Its fixed test configuration is now eight seconds cumulative
and ten seconds per invocation; it must still reach one invocation, time out, refuse
adoption/retry/certification and finish before the invocation allowance. Production
defaults and resource maxima did not change; no runtime backoff is introduced.
