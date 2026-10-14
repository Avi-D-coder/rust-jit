# T031 loop and control transfers — work in progress

Original loop now enters its lexical scope and body, checks unit on ordinary body
completion, retires iteration storage and emits the same explicit backedge used
by while. Break/continue exit before ordinary body completion. Labeled while,
loop and plain blocks retain their owned structural target identities.

Reached target selection walks the structural lexical target chain. Unlabeled
break/continue skip non-loop block targets; named labels use canonical original
source spelling, with the nearest matching label shadowing outer labels. Continue
requires a loop target. Missing labels and break values for while fail before
entering the value expression. Macro hygiene and generated/source-view routing
remain later integration; runtime tags do not select source labels.

A break recipe owns its selected target, optional value slot and explicit resume
PC. Stable encoding includes that continuation. The verifier admits only lexical
ancestor targets and valid continuation PCs; continue additionally requires a
loop. The VM checks that the target is active, moves the result into a checked
ticket, then retires inner scopes through the target scope. It initializes the
target expression result in its enclosing scope and resumes the parent's phase.
The result ticket survives retirement of the source slot.

Continue retires inner scopes through the chosen loop scope and yields a distinct
backedge outcome to its entry. The dispatcher records that backedge and reuses
installed recipes. Intermediate body bindings/temporaries are cleared; outer
locals remain active. Cleanup here is the existing scalar/storage implementation,
not general guest Drop or reference-lease acceptance.

Four added controls cover loop break values, repeated continue with iteration
locals, while break/continue, nested labeled transfers, labeled block break,
shadowed label identity, unlabeled break across a labeled block, break inside a
break-value expression, invalid targets before evaluation and malformed IR.
The legacy T030 modifier-guard fixture retains its name/unsupported modifier
checks and now also has a positive labeled-block regression. Labels have explicit
control semantics instead of the earlier blanket guard.

The first build failure is retained: the new helper initially used an incorrect
local structural-node type name. It was corrected from the existing source; no
frontend/API fork or specification erratum was needed.

T031 remains in progress. Successful scalar per-op/exit fuel and cancellation/
deadline polling are now exercised in [the fuel checkpoint](run-fuel.md).
Uniform runtime preflight, cleanup/aggregate element charges, full interruption
integration and final acceptance audit remain unfinished. General references/user Drop/
pattern/if-let/generated/hygienic/source-view/epoch/JIT and Gate03 integrations
remain later cross-ledger work. No selected guest unit compilation, native symbol
loading or macro execution authority is introduced. Full release/performance/
security evidence and actual Linux acceptance remain pending.
