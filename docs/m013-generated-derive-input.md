# M013 generated derive input materialization

Revision 4 Sections 49–51 and Section 57 milestone C, macOS ARM64 with Rust
1.99.0. M013 remains in progress. The invariant is that cfg preprocessing and
removal of certified derive annotations preserve retained original tokens, spans,
contexts and item-local helper scope. Materialization is pure data and grants
neither procedural execution authority nor a completeness certificate.

The exact output/header/cfg/epoch-bound plan checks its input again before
materialization. Recursive cfg facts are separate from outer ADT/member helper
facts: nested items, generic parameters, tuple fields, expressions and match arms
can be configured without borrowing outer helper ownership. Inner cfg attributes
on function bodies configure the owning function. Disabled owners suppress their
descendants. Effective cfg_attr metas reuse their original token forests.
Unresolved helper and replacing attributes remain pending.

The pinned syntax bridge selects AST owners and removes disabled subtrees and
commas. Its existing leaf injection hook retains the original lexical occurrences,
including punctuation spacing and literal spelling. Attribute edits use occurrence
ordinals rather than span identities: repeated tokens may share spans while needing
different edits. Coordinates select syntax only; no fabricated source spans or
alternate parser is introduced. Bounds cover metadata, tokens, depth and wire size.
Invisible-group equivalence across the whole production pipeline remains part of
full fixture acceptance.

Seven unchanged official rustc cases at
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18` retain auxiliary crates, directives,
snapshots, MIT and Apache-2.0 license texts, and COPYRIGHT in
[the copied directory](../fixtures/upstream/rustc-checkout/README.md).
The new issue-75930-derive-cfg case exercises recursive cfg preprocessing.
Component harnesses use a labelled identity-macro envelope outside that directory.
The envelope's captured `<` spacing is Alone; rustc's original snapshot reports
Joint. The component checks preservation of the actual captured token, not full
snapshot equivalence. The failed spacing assumption and compile/API witnesses
remain in the checkpoint artifacts. Full native/compiletest/LRR acceptance for
these seven cases is pending; expand-to-derive is still unexercised.

The existing authorized native macro-chain control now feeds the materialized
second attribute output to two sibling derives. Four native invocations, two
replacement reanalyses and two appended outputs match its native Rust reference.
The control retains separate preparation/execution authority, matched external
server and exact input adoption. It is a synthetic SDK/coordinator comparison;
it does not exercise production SourceDiscovery helper classification, guest
execution or full publication. Its report and input wire retain hashes.

Twenty focused derive controls cover facts, selection/replay and materialization.
New controls reuse cfg/helper/nested/issue-75930 upstream sources; a separate
LRR-specific shared-span regression checks occurrence editing and stale-input
refusal. Final gate results are recorded by the checkpoint, not inferred here.

Next work is production discovery/coordinator suspension, compiler facet
installation, exact derive export/helper declaration demands, authorized execution,
ordered appends and final publication. Complete transformation/world/edition/
Cargo/CLI/service fixtures and later M014–M016 gates remain required. This component
does not establish traits/JIT/native/security/performance, release or Linux support.
