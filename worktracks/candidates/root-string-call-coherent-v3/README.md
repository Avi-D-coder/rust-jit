# Coherent string literals, calls, patterns and equality

This 31-leaf packet supersedes `root-string-call-coherent-v2` and its `root-string-patterns-v1` follow-on. Apply this combined form once after the byte milestone, without also applying constituent packets. Exact joins and every base leaf were checked against frozen V87 `2e19361d`; its validated owner-depth/cycle repair remains intact.

The union covers literal decoding and immutable UTF-8 storage, ordinary and macro literal execution, source-selected equality in macros and ordinary expressions, shared string arguments through parameter delivery/read, and literal string patterns with bounded comparison continuations. The new pattern phase is encoded in body-structure domain v5. Source type, owning parse/profile, current member, physical reference and exact PC/slot checks remain required.

Pending unchanged whole-main witnesses are `slowparse-string.rs`, `underscores-in-unicode-escapes.rs` and `match-borrowed-str.rs`, across all editions. Other original-derived controls remain component checks. Source aliases, methods, owned strings, mutable/general borrowed views, reference returns and general string pattern bindings remain outstanding.

Formatting, ordered source joins and exact replay are static evidence. Rust compilation, all runtime controls and the integrated milestone are pending. No stock runtime/full-fixture pass, broad task completion or guest JIT progress is claimed. Retain newer canonical upstream originals and merge inventories additively when preparing validation.
