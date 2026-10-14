# M016 preflight — origin chains and discovery diagnostics

This is a review for the next ordered task, not implementation or acceptance.
M015 must first pass its integrated macOS milestone.

The current Diagnostic owns only code/message. Recipe Origin retains a BodySpan
and expansion ancestry, but RunPoll faults and CLI rendering do not yet provide
a complete generated method/invocation/definition/include chain. M014 preserves
owned generated token origins. M015 records actual producer chains for generated
test entries. Reuse these records; do not recover source positions by parsing a
rendered expansion or treating syntax-context marks as producer parents.

Before changing code, add a failing origin acceptance control using the original
rustc fixtures as syntax/procedural input sources where relevant. A reached
unsupported generated operation must identify the actual generating invocation,
source file and checked span. After an edit, stale source bytes or prior line
positions cannot be emitted as current evidence. A body-only edit and a header
edit must have distinct invalidation observations under MX034; selected trait
method behavior remains dependent on the later trait tasks.

Preserve the existing small Diagnostic type where appropriate and attach bounded
owned origin evidence at the selected failure boundary. The bundle must name
content/source identities, checked ranges and producer records, carry no pointers
or worker/session handles, and redact unauthorized host paths and source content.
Include-origin facts already expose adopted tokens, source blobs and dependencies;
use their freshness checks instead of accepting a cached display string.

Pending full acceptance: MX033/MX034 and integration with traits, cache remapping,
real guest JIT, service/CLI schemas and the final fixture oracle. A metadata-only
origin mapper or controlled native macro failure is not a full guest diagnostic
or cache-invalidation pass.
