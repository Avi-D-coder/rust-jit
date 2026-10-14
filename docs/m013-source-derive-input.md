# M013 ordered original-source derive inputs

`derive_occurrence_resolved` builds an immutable original-source occurrence after
replaying the exact compiler annotation proof against complete source/cfg/member
facts. It removes certified groups through the selected annotation and preserves
later groups and replacing/helper attributes. Paths in one group share an item
payload but bind different lexical ordinals and original path spans. The input
has a distinct stable encoding domain and checked retention. The request factory
accepts this immutable occurrence identity rather than a separate caller ordinal.

The prior whole-item `derive_resolved` helper remains available for its existing
all-certified-annotation use. This new ordered input is a separate contract; it
does not classify helpers, authorize native execution or settle discovery.

The unchanged official `derive-expand-order.rs` source and `.stdout` provide five
complete expected payloads. A regression failed before the new implementation:
the first payload incorrectly removed Second, Third/Fourth and Fifth. It now
compares every token, delimiter, raw identifier and punctuation spacing, with
original source/context records checked separately. It also checks distinct
occurrences, same-group payload equality and refusal of changed later annotations,
owner, cfg or ordinal. A supplementary LRR-specific adaptation repeats Third,
adds a later attribute and cfg-filtered members; both repeated occurrences preserve
the same payload, keep later tokens and reject stale member facts. Adaptations
are outside the licensed upstream directory.

An explicitly prepared, unchanged original `multiple-derives.rs` auxiliary then
exposed [E-M013-001](errata/m013-procedural-stdout.md). Its first invocation executes
the print but corrupts the matched binary response stream. The retained positive
probe fails; the terminal-refusal control checks that exact failure and prohibits
further native attempts. The fixture's procedural execution remains failed, with
zero completed expansions and four unexecuted occurrences. It is not counted as
another successful stock case. No runtime source was compiled or fake expansion
record admitted.

M013 remains in progress. Original-source helper classification, worklist execution
demand, authorized MacroRun dispatch and ordered append/publication integration
remain required. The stdout compatibility repair, complete integration matrices,
full compiletest/tier/JIT acceptance, broader traits/runtime, release and actual
Linux acceptance also remain pending.

The integrated macOS component gate observes 1657 host/doc test passes and all
704 registered M013 controls. Gate00/01/02 and provisional Gate03, strict Clippy,
formatting and architecture audit pass. These include successful input assertions
and the specifically scoped refusal regression; they do not accept the failed
procedural fixture. See [the checkpoint](../artifacts/macos-first/m013-source-derive-input-checkpoint.json).
