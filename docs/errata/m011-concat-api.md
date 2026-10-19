# E-M011-001: syntax-only pinned concat entry

The pinned eager concat calculation does not use its SourceDatabase or MacroCallId
parameters, but its public dispatch path requires both and performs source-driven
hygiene lookup. The adapter needs the calculation under explicitly owned tokens
and a registered caller span, without entering source queries or constructing a
HIR macro call.

Extract the calculation into a private helper, preserve the existing
expander's quote/error behavior, and expose `concat_value_tokens` returning an
owned string or its original expansion error. No matcher, evaluator or backend is
replaced. Two audited patches expose/reexport this entry from the original archive;
the pinned archive and inventory remain unchanged. A source-proof regression checks
the entire original calculation body, allowing only its returned tuple.

The actual `concat!(0x10)` native comparison confirms a numeric-spelling gap:
pinned output is `0x10` and native output is `16`. See
[E-M011-002](m011-concat-numeric-spelling.md). The pinned numeric correction has native matrix acceptance; complete literal/error
semantics remain pending. API extraction itself changes no source-query semantics.

## Syntax-only byte concatenation (October 18, 2026)

The same adapter boundary now exposes `concat_bytes_tokens`; the original pinned
RA compiler dispatch delegates to that hook. The archived byte expander handled
literal arrays but omitted repeat arrays and silently omitted radix or suffixed
integer elements. Its corrected compiler array helper uses the existing literal
number conversion and byte unescaper, checks host-size arithmetic, and emits the
same byte-string token representation. The governing Rust contract is the pinned
`rustc_builtin_macros/src/concat_bytes.rs` `handle_array_element` and
`expand_concat_bytes` implementation; the unchanged `tests/ui/macros/concat-bytes.rs`
is the primary runtime witness. This remains explicit compiler preparation;
returned tokens enter the ordinary source grammar and VM. It is not a general
expression evaluator or macro-specific runtime path.

The existing archive-derived patches for fn_macro.rs and builtin.rs are regenerated
in full with exact preimage/postimage receipts. The pinned archive, commit and
inventory stay unchanged. Validation is pending until recorded in the bounded
worktrack evidence; no complete diagnostics, byte-macro or interpreter gate claim.
