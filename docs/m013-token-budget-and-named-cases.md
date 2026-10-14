# M013: cumulative token admission and named discovery cases

Revision 4 / Section 57 milestone C; macOS ARM64 Rust 1.99.0 first.
M013 and release acceptance remain incomplete.

Invariant: all expansion records admitted to the same source discovery request
share Section 50's maximum of 2,000,000 generated tokens. Refusal must precede
header/catalogue integration and must not remove the existing pending demand or
charge an output that was never admitted.

ExpansionRecord derives a checked token-tree node count from its schema-validated
wire at construction. Groups, including invisible transport groups, count as one
node; nested children are counted recursively with the existing bounded depth.
The cached count is internal derived metadata: wire encoding and semantic identity
remain unchanged, and record copies preserve the count.

SourceDiscovery checks cumulative token count before any source, generated child,
physical-module or adopted compiler output can integrate. The counter commits only
after successful admission. Compiler admission checks the limit before retained
capture/stamp metadata changes. Existing exact-request, failed-request and sealed
request diagnostics retain precedence. A request builder may explicitly lower the
limit before polling; it cannot increase the specification maximum or change the
limit mid-request. CLI/service resource override integration remains pending.

Four controls force refusal of a later original output, shared parent/generated
child exhaustion, invalid/late configuration, and the exact empty transport-group
boundary. The initial assumption that empty macro output had zero nodes was wrong:
the actual adapter produces an invisible group. Its failed log is preserved; zero
budget refuses that actual output and a budget of one admits it. No fabricated
empty record substitutes for the actual adapter result.

Two direct named discovery controls exercise MX027's generated import/macro fixed
point and MX028's unresolved producer cycle plus an originating cyclic macro
reexport. Successful paths are compared with isolated native oracles. The initial
explicit reexport fixture duplicated an import under native Rust; its rejection
is preserved and the shared source uses a valid cyclic glob-reexport setup.
These controls do not replace M043's complete release fixture reporting.

Section 50 audit still requires broader producer cycles, nested impl/test catalogue
integration (M014/M015), origin coverage (M016), complete resource/cancellation/
supervision and service integration. Intermediate compiler string-operand records
and procedural invocation accounting need complete request-level integration;
this change counts source-discovery admitted outputs. Body-position user expansion,
comparison assertion operands, type/JIT/security/release acceptance and Linux remain
pending. M013 is not marked complete based on these component checks.
