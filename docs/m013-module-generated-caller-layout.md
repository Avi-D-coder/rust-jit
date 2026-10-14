# M013 retained generated caller layout

M013 remains in progress. The standalone coordinator can now prepare an external
module emitted by a later invocation inside generated inline modules. It obtains
the actual catalogue and its retained ancestors from discovery, validates their
source ordinal, owner scope, cfg, compile environment and expansion descriptor,
and walks the frames in order from the original caller to the current invocation.
The root physical layout comes from the original captured inline syntax. Each
additional physical suffix comes from the actual enclosing generated items in its
parent frame, using the existing pinned parser and canonical-name/module-attribute
rules. A logical path without the corresponding retained syntax is refused.

Empty same-scope frames contribute no invented directory. Inline path attributes
and ordinary directory components retain their physical file-relative versus
module-directory rules when combined. The resulting layout has checked depth,
component metadata and allocation limits. Existing request construction retains
the original generated declaration and hashes the actual candidate paths and child
directory. The public lower-level preparation API continues accepting an explicitly
supplied caller directory, preserving generated sibling API clients. Query providers
perform no filesystem access, build execution or macro execution; the existing
phase-bound coordinator performs capture, reply acceptance and adoption.

Seven new CLI controls cover a later generated caller, original and generated path
attributes combined, multiple nested expansion frames, generated siblings with
separate physical directories and source-order macro cutoffs, a nonroot original
file, cfg-selected Unicode/raw names, and an authority escape. Six successful cases
compare actual CLI values with explicitly compiled isolated Rust1.99 macOS oracle
copies. Competing files in incorrect directories expose fallback errors. The
escape requires a specific source-path refusal with empty stdout. The initial
missing-layout failure is preserved in `m013-module-generated-layout-before.log`.

Nested adopted physical file-module and included-source caller contexts, child
compiler/body macro routing, service coordination, namespace facets, cache/quota
boundaries, full MX027/MX028 and JIT/release acceptance remain pending. Linux is
a later port; these controls establish macOS evidence only.
