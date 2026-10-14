# M013: procedural calls in generated item output

Revision 4 Sections 50–51 and fixed Section 57 milestone C. macOS ARM64
Rust 1.99.0 first. M013 and full release acceptance remain incomplete.

Invariant: generated function-like procedural calls retain the actual token forest,
call scope and parent expansion origin, resolve against the prepared compiled export
facet, and execute only in the owning MacroRun consumer session. Pure generated-call
assembly and namespace lookup cannot execute helpers or certify complete membership.

Generated macro lookup now includes the owning prepared procedural export routes.
Pure request assembly strips the argument delimiter while preserving token spans,
contexts and provenance. It records the exact compiled export identity, epoch,
consumer environment and procedural engine identity. Captured source callbacks are
selected from the owning provider and retained source providers by actual span source
identity; unavailable source bytes are never fabricated. Source counts and input,
capture and pending-request metadata are bounded with checked sizes.

The run checks the exact pending generated demand, shared budget ownership, matched
session catalog and recomputed native request key before effects. Native output must
already be adopted by the coordinator. Direct output admission and pure expansion
refuse. Outputs use the existing generated child worklist, so their declarations,
imports and nested macros participate in completion and final selection revalidation.
Final actual-input verification and atomic publication remain required.

Actual-server controls cover two and three levels of procedural invocation followed
by a generated declarative macro and value import. They require one consumer session,
exact invocation/output accounting, unadopted-output refusal and successful sealed
publication. A generated conflicting import of the actual session_counter export
requires LRR-E-MACRO-AMBIGUOUS after two native calls and no seal/publication.
These are component controls, not complete Rust execution or fixture acceptance.

Initial failures are retained in the focused logs: missing procedural export routes
left the generated call pending; native output admission used an overly generic
diagnostic; passing the outer argument delimiter caused invalid item output.
Mechanical API mismatches were repaired using the existing provider/token APIs.

Attribute/derive discovery, complete cross-unit/world/edition and Cargo matrices,
generated calls combining procedural expansion with all include/module forms,
complete CLI/service execution, M014–M016, traits/JIT/native runtime/security,
performance, reproducible release and Linux acceptance remain mandatory.
