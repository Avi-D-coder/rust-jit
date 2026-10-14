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
