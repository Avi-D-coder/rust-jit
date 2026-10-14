# M013 original derive-to-attribute continuation

Task: M013, revision 4 Section 50 discovery and Section 57 ordering. The invariant
is that an original item's following replacing attribute may run only after all
preceding derive occurrences have completed append discovery. Earlier output stays
published in the discovery graph; replacing the original item must not erase it.

The acceptance case uses the second item of the unchanged official rustc
`tests/ui/proc-macro/attribute-after-derive.rs` import. The copied case, auxiliary,
directives, stdout and original licenses remain unchanged. Outside that directory,
the LRR test selects that item, substitutes SDK checking exports, expresses false
cfg as `any()`, and adds the SDK helper and `repr(C)` input assertions. This is
adapted component evidence, not a full execution of the stock fixture.

The before log records the actual unresolved-producer failure after the derive
append. The coordinator now replays completed occurrence identities, namespace
selections and helper receipts before issuing an immutable attribute selection.
The continuation fingerprint binds the preceding classification and completion
proof, selected input, definition and prepared catalog. A private source attribute
receipt must match at request binding. Seal revalidation replays the preceding
derives again and compares the complete attribute selection fingerprint.

Attribute materialization removes only the applied attribute and preceding
certified derive annotations. Raw member tokens remain, including disabled member
cfg. Derive input still uses configured members. The existing replacing-output
path handles the actual attribute output; no original declaration is restored
before replacement. No helper executes in a query provider.

Three targeted macOS controls pass: the derive and replacing attribute execute
once each and publish the earlier constant plus replacement type; a macro emitted
by the derive remains usable by a later original invocation after replacement;
and a source edit refuses attribute dispatch before a second native invocation or
seal publication. Gate00/01/02 and provisional Gate03 component now pass with 1680 host/doc
tests and all 727 M013 controls. Formatting, strict Clippy for runtime and host
tool, architecture and first/final manifest audits pass. The licensed import retains
35 component assertions across thirteen cases and 32 verified files. The
restricted unchanged empty-derive interpreter/native reference was rerun; full
compiletest/LRR cases and guest JIT acceptance remain zero. M013 remains in progress. Full ordering/lint/helper diagnostics,
resource/world/edition/service/CLI matrices, guest JIT and Linux acceptance remain
pending. In particular, original-source helper inheritance through an attribute
that forwards the item still needs integration and acceptance; the current
replacement control does not claim that behavior.

Evidence: `artifacts/macos-first/m013-following-attribute-before.log`,
`m013-following-attribute-controls.log` and [the integrated checkpoint](../artifacts/macos-first/m013-following-attribute-checkpoint.json).
