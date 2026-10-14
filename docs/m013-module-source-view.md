# M013 adopted module declaration views

M013 remains in progress. An explicit module request now retains the actual
ModuleRequest declaration and encodes its logical path, directory suffix,
file-relative flag, name and path override in the stable request identity. Two
requests for the same physical candidates with different logical declarations
have different identities. Declaration allocations participate in checked metadata
accounting.

SourceDiscovery prepares a module input from the retained generated header,
expansion, consuming compile environment and actual invocation-file base. The
caller supplies the directory recorded by explicit source preparation. Retained
virtual include bases require further physical-layout integration and currently
return a typed incomplete diagnostic; this path is not claimed supported.

Pure module view lookup re-observes the caller source selection, cfg and unit
compile environment, checks the declaration and candidate invocation origins,
and suspends until coordinator adoption. Successful lookup retains the adopted
input lease, exact parent generated syntax and hygiene, original caller publication,
and invocation-file span. It structurally parses the frozen child bytes in a fresh
logical child scope with the captured caller edition and cfg. Owned view metadata
has a checked limit. It performs no filesystem capture, macro execution, function
body lowering, namespace publication or producer discharge.

Three registered controls cover adoption separation and an unresolved frontier;
logical-declaration substitution despite identical physical candidates; exact
invocation-file retention; frozen lookup after file deletion; changed caller or
compile environment; nested logical scopes; and missing/ambiguous adopted source
evidence. The cold function containing unresolved types is retained structurally.

This is a declaration-view increment. Correct parent macro visibility and textual
position, child declaration/import/macro scheduling, derived source publication,
recursive external modules, included-file physical layout, final frontier closure
and actual LRR child execution remain required. The historical positive generated
module witness remains pending with comparison_passed=false. Full MX027/MX028,
JIT and release acceptance are unfulfilled. macOS ARM64 Rust1.99.0 is the current
validation platform; Linux is deferred.
