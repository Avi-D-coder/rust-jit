# M013 original-source derive append integration

An authorized MacroRun can integrate the exact output retained by an original-source
derive demand. The run checks its frozen source revision, separate authority,
shared expansion budget, own output receipt, captured files and adopted database
record. Forged output, foreign runs, duplicate append and source changes refuse
integration. Appending invokes no native macro and runs no build tool.

The real expansion becomes a generated-source fragment in the existing discovery
pipeline. It retains the original captured-source lineage and supports nested
generated macro calls. No artificial ExpansionRecord represents the original item.
Wire/header/catalog/source-prefix metadata has checked cumulative limits. The
original job blocks while this fragment is live; membership changes cannot let a
later derive overtake unfinished nested output discovery.

On fragment completion, outward macro bindings enter the original producer's
textual prefix without retiring that producer. Each fragment retains a preceding
source snapshot for selection replay and route construction; its own and later
outputs cannot enter that snapshot. Completed occurrence selections replay their
exact input, definition and receipt fingerprints when selecting subsequent work.
Same-group occurrences share their item payload but have distinct identities;
later groups receive the item with preceding derive groups removed. Helper scope
uses exact receipts from the current and preceding groups, with source context
and bare-name checks. Helpers from later groups are not granted early.

The regressions adapt the copied, unchanged rustc `derive-helper-configured.rs`
outside its licensed directory. They select controlled fixture exports, use the
fixture's helper name and cfg(all()), add repr(C), and rename the source item to
request nested output from the controlled macro. The ordered case adds a second
occurrence in the first group and a later group. Actual macro output is parsed
and discovered; the nested function is produced by expanding its actual emitted
macro body. The ordered case requires that nested output to finish before the
second derive selection. A separate source-revision regression executes once and
then refuses stale append with no additional invocation.

These are component controls. Final processing of the remaining original outer
helper/attribute annotations, source declaration restoration, complete publication
replay, legacy/later helper cases and full service/resource/world/edition matrices
remain pending. M013 is in progress. E-M013-001 still prevents the printing rustc
auxiliary from completing. No new full compiletest, guest JIT, release or Linux
acceptance is claimed.

The eight targeted original-source derive tests pass. The integrated macOS gate
records 1664 host/doc passes and all 711 M013 controls. Gate00/01/02 and provisional
Gate03 component, strict Clippy, formatting, architecture and first/final
fixture-manifest audits pass. Evidence is recorded in the reproducible
[append checkpoint](../artifacts/macos-first/m013-source-derive-append-checkpoint.json).
