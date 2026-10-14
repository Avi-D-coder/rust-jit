# M013: unchanged rustc printing derives through source discovery

The selected case is the unchanged copied `derive-expand-order.rs` and unchanged
`auxiliary/multiple-derives.rs` from the official checkout. Explicit Cargo
preparation retains the original dependency alias, derives and star import,
compiles only the 2015 macro host, and records zero live compiler invocations.
The consumer remains a 2024 live unit. The registered prepared server is admitted
through the same typed provenance and transaction/freshness contracts.

Discovery can now atomically install prepared procedural facets before constructing
declarative extern/import routes. The catalog must match graph, epoch, consumer and
compile environment. Metadata configuration executes no macro or build tool.
The earlier chained configuration remains supported for sources that do not need
those aliases during initial route construction.

The auxiliary defines its five derive registrations through a local macro_rules
producer. On demand, the coordinator reads only captured macro-host and consumer
roots, creates declaration-only source facets and invokes the pinned pure
declarative expander. Generated syntax replays the existing owned syntax events;
bridge text intentionally omits separator whitespace and is never re-lexed.
Bodies remain opaque. The complete registration catalog must exactly match the
loaded native derive-export set before atomic helper admission. Unknown,
transforming, nested or macro-binding-producing facets remain pending rather
than certifying empty helper metadata. Direct metadata admission still suspends
on producers; this demand path is separate.

Generated declaration proofs bind producer/root bytes, cfg, edition, actual
expansion descriptors and declaration headers. Retention and invocation/output
counts have checked bounds. Repeated occurrences reuse the admitted source facet
only after current input/freshness checks. One actual host declaration expansion
serves all five actual native derives.

Source derive requests now carry a captured macro-host root definition-site span
and callback source, with producer identity and edition. The call/mixed sites
remain in the consumer context. The root anchor is not a claim of precise generated
function diagnostics; origin rendering remains later work. This unchanged fixture
exercises definition edition 2015 and call edition 2024. Source and artifact hashes
remain checked; no native export spelling is used to infer helper emptiness.

The worklist requires First, Second, Third, Fourth and Fifth once each, distinct
occurrence keys and append-before-next selection. All real outputs are empty,
all five records settle, Foo keeps its canonical source identity, and its sealed
name query becomes ready only after owning MacroRun publication. Closed-run
diagnostics retain bounded protocol/stderr previews and truncation flags;
reading starts no helper and query-provider access is refused.

Five actual diagnostic payloads are compared with unchanged upstream stdout
and actual occurrence inputs as complete token shapes, including punctuation
spacing. Pretty-print whitespace is not an exact stdout compatibility claim.
This is original-source service/worklist component acceptance, not guest main
execution, full compiletest/LRR tier acceptance or JIT acceptance. Full CLI,
replacing-attribute, generated macro-host/nested/procedural/include and broader
M013 resource/world/edition matrices remain pending. Linux is unvalidated.

The integrated macOS checkpoint has 1677 host/doc passes, all 724 M013 controls,
and 34 component assertions across 13 imported rustc cases. All 32 imported file
hashes pass verification. Gate00/01/02 and provisional Gate03 component, formatting,
strict Clippy for runtime and external host tool, architecture and manifest audits
pass. Full compiletest/LRR cases and guest JIT acceptance remain zero. The separate
restricted empty-derive interpreter case is retained.

See [the checkpoint](../artifacts/macos-first/m013-original-printing-worklist-checkpoint.json).
