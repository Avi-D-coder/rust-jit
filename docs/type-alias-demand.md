# T032 type alias demand — work in progress

TypeAlias is a separate pure query over existing header, source and declaration
context inputs. It captures the exact alias AST declaration, defining scope,
generic parameter/bound/default facts, target syntax and known rigid structural
terms. Owned capacities and term budgets are checked; stable tags distinguish it
from headers and function signatures. Shared generic capture retains lifetime/
type/const namespaces without solving their bounds.

The normalizer uses the existing checked immutable source namespace graph and
published database source locators. It walks only reached declarations with a
256-declaration limit, validates the selected source identity, and resolves each
single-name target in the alias's own scope. Primitive fallback follows source
absence; unknown producers, ambiguity, missing names and owner contexts remain
explicit outcomes. A nominal target is an identity demand, not an assertion of
empty generic arguments or a finite layout. Fields are never expanded here.

A repeated identity faults with LRR-E-TYPE-ALIAS-CYCLE and its reached path.
Cycle detection never invents unit/absence. Pure owned alias recipes reuse the
existing memo framework. A depth fault precedes the next alias fetch. The controls
exercise primitive chains, differing caller/definition nominal scope, cold invalid
signatures/cycles, memo reuse, producer/generic deferral, stale source rejection
and depth exhaustion. No function body/thunk query is needed by expansion.

This is initial demand expansion. Generic alias argument instantiation, defaults,
qualified/composite target recursion, projection/opaque/function/binder/region
normalization, concrete nominal layout and complete SourceView/RuntimeSubst
integration remain mandatory. Retained deferred target syntax is not a successful
concrete contract. Type alias clauses may also require later generated discovery
or goal input; this code never executes macros, guest code or build tools.
T032 remains in progress pending actual source calls, argument phases, sealed
packets and boundary acceptance. macOS is primary; release/Linux remain unverified.
