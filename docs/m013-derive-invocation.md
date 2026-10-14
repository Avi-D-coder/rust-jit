# M013 authorized generated derive invocation and retained output

Revision 4 Sections 18.2, 49–51 and Section 57 milestone C, macOS ARM64,
Rust 1.99.0. M013 remains in progress. This step executes selected derives in
source order while keeping append admission and publication incomplete.

The immutable helper proof now retains the exact selected definition IDs and
prepared catalog fingerprint. A derive request requires that definition and
occurrence, a current item/header/cfg/epoch binding, and no unresolved outer or
member attributes. Unknown attributes and deprecated before-derive candidates
cannot be forwarded as settled input. Repeated derives keep separate occurrence
identities even when their item token trees are identical.

Request assembly uses the existing bound materializer and procedural client. It
selects ProcForm::Derive with item grammar, exact export/catalog, edition, parent
origin, captured source callbacks and epoch. Materialization removes derive
annotations from the item tokens. The request separately retains the original
annotation's site span and context rows so the matched protocol can register the
call site. Those metadata rows do not restore an annotation to the item. Explicit
stable input fingerprints include the different sites. Query-side expand refuses
procedural execution; no build tools or new native symbol loader are introduced.

The existing MacroRun fulfil_generated_discovery path checks macro authority,
frozen inputs, shared budget, matched catalog and session request key before
execution. Its normal validated output is adopted in the database. Discovery then
requires the exact pending derive request and adopted record, parses item grammar,
preflights checked wire/allocation limits and immutable worklist dependencies, and
retains the output against that occurrence. It retries the same producer rather
than completing its frontier. The next unresolved occurrence runs next.

Retaining output does not replace the annotated item or publish generated items.
After all derives execute, discovery still waits on the first occurrence until
append integration, revalidation and catalog publication are implemented. Existing
pending-only controls explicitly defer the new derive request; they still assert
incompleteness and are not full fixture passes or skipped successes.

The pure request acceptance checks ordered identities, equal item token trees,
distinct site-sensitive fingerprints, registered call-site metadata, derive form,
query execution refusal, invalid occurrence and unresolved-attribute refusal.
The actual matched-server chain executes CheckDeriveInput and Empty after two
replacing attributes. CheckDeriveInput checks the actual retained repr/helper
attributes and emits a const; Empty emits no items. The control records four native
invocations and two retained derive outputs in lexical order, proves the original
parent descriptor unchanged, and rejects unadopted and duplicate outputs. It also
checks that sealing and publication still fail. Actual wires, keys and parent/output
identities accompany the checkpoint. The initial missing-API compile, a test-harness
access correction, and the missing site registration failure remain recorded.

These are LRR-specific execution/authority controls. The eight unchanged licensed
rustc cases, fifteen component assertions over seven cases, and all pending full
native/compiletest/LRR acceptance states are unchanged. The Rust test sources remain
the primary language fixture source; preserve their companions and original terms.

Next work is ordered append integration with original-item release, output imports
and nested producers, final positive replay and publication. Later replacing
attributes, complete scopes/editions/worlds, original-source and builtin payloads,
pure/source parent provenance, generated macro-host declarations, complete Cargo/
CLI/service matrices, later tasks, full fixtures, release and Linux acceptance
remain mandatory. This component does not complete M013.
