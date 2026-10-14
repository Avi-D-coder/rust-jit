# T032 function signature capture — work in progress

FunctionSignature is a separate demand query layered on the existing immutable
header/source inputs. It locates the original declaration by exact header tokens
and its published body range where present, without creating a body index or
thunk. Signature(Digest) continues to return the declaration header for existing
consumers. The new result binds its exact definition and uses an explicit stable
query tag and recipe encoding; owned capacities have checked byte accounting.

The owned recipe records lifetime/type/const definition parameter ordinals,
canonical type/const names, bounds/defaults/const parameter types, receiver and
written parameter syntax, declared result and where predicates. Header context,
profile and modifiers remain retained. These are symbolic source facts. Capture
neither resolves an unknown type nor proves a bound, and it never compiles code.
Known declaration-parameter/aggregate terms now have
[structural substitution](structural-signature-substitution.md). Unresolved
paths/projections and SourceView/ParamEnv integration still need work.

Actual demand tests assert that unknown ordinary bounds and a cold unknown
function signature cause no body/thunk/name/constant query. A body-only edit
preserves the semantic result fingerprint; a bound edit changes it. Repeated
parameter identity faults explicitly before body entry. Attribute transformations
retain the existing pending-integration guard rather than admitting an unexpanded
signature as a final call contract.

T032 remains in progress. Source calls do not yet consume this query; argument
phases, concrete signature boundaries, structural generic substitution, owned
sealed CallPackets and return checks remain required. These query tests do not
prove call execution, ignored-bound behavior at an actual call, argument effects,
macro suspension, native conversion or release acceptance. macOS evidence is
primary; Linux acceptance remains deferred.
