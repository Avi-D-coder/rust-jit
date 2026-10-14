# M011 original-token environment arguments

The adapter parses env/option_env argument groups through the pinned token/syntax
bridge and expression parser. It decodes raw and escaped Rust strings through the
pinned literal API and lexer, with original literal span keys. Env supports a name
and optional custom missing-value message; option_env permits only the name. Tail
commas and macro delimiter forms are retained. Missing/interior-empty/excess
arguments, non-string expressions, invalid escapes, string suffixes and oversized
decoded values refuse.

A nested macro expression produces a typed `NeedExpansion` with its original
endpoint spans. It is never evaluated as a guest expression or guessed to be an
environment name. Arguments retain their exact builtin descriptor and a stable
fingerprint over owned original tokens/context/span records.

The runtime copies owned operands across the adapter boundary and requires the
matching descriptor at evaluation. Nested operands suspend in lexical order before
environment lookup, including a custom-message operand when the name is present.
Uncaptured names yield a span-bearing capture demand. Explicit absence reports the
custom diagnostic when supplied; optional absence yields None. No resolution or
evaluation method executes a macro, reads ambient environment or opens a file.

Acceptance checks original raw literal spans, raw/Unicode escapes, delimiter and
tail-comma handling, invalid input rejection, nested name/message suspension,
descriptor mismatch, uncaptured names and custom diagnostics. A Rust 1.99.0 macOS
oracle compares escaped required names, raw optional absence and custom Unicode
missing-message diagnostics.

Nested expansion fulfilment, coordinator/query capture adoption, constant concat
evaluation, include paths and immutable authorized source capture remain pending.
This component does not complete M011, MX021–MX024, guest execution, release or the
later Linux port.

FrozenCompileEnvironment::expand_environment now binds exact compiler selection,
original tokens, definition edition, expression grammar and captured environment
fingerprint before output. Required env values emit records through the pinned
string quote/owned span transport. Unknown names and nested operands stay distinct
owned capture/expansion demands; explicit required absence preserves a custom
missing diagnostic. Changed environment and mismatched key/edition refuse.

Option values and absence are retained as NeedOptionFacade with the exact defining
unit and source site; output is not fabricated from Some/None spellings. Facade
fulfillment remains pending. Native quote/Unicode/newline env output agrees with
the decoded emitted record. This is env string emission component evidence, not
complete option output, automatic environment-input adoption, recursive eager
evaluation or guest acceptance. Concat output also checks definition edition.
