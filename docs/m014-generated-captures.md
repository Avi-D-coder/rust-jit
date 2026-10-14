# M014 generated macro token captures

M014 remains in progress. This integration captures generated `macro_rules!`
bodies and nested macro paths/arguments through the pinned syntax bridge's
span map. It never lexes display text, prepares ordinary operations, grants native
authority or retires a producer frontier.

The adapter visits the actual parser nodes, keeps macro token trees opaque and
exports owned trees with their original span/context records. Definition captures
retain the real name span and keyword-context edition; call captures retain the
actual path, argument group, invocation site and edition. Counts, retained token
memory and allocations have checked limits. The runtime matches capture ranges
to the cfg-active declaration inventory and indexes them by its exact identities.
Unknown identities return a typed input error; disabled declarations are not
exposed as active captures. Capture storage participates in metadata accounting.

Thirty-two targeted macOS controls pass. One feeds a captured generated definition
and call into the real pinned declarative matcher/transcriber and inspects its
actual impl/member tokens. This is an unmarked component transcription check,
not final hygienic resolution or publication. The unchanged rustc statement
fixture's captured `foo!()` retains its real argument group and marked contexts.
Other controls cover actual literal arguments in expression/pattern/associated
slots, malformed unused definition bodies, scalar output and opaque macro payloads.

A regression found a disabled macro statement surviving cfg. The pinned parser
can attach its attributes to nodes omitted by AnyHasAttrs. The scanner now uses
direct Attr children, consistent with the trait's structural ownership rule, and
applies effective cfg before descending into both source and generated statements.
The actual failure is retained; this is an implementation correction, not a
frontend or language-profile change. A malformed test fragment lacking its
required associated-item semicolon was corrected outside the licensed fixtures.

Gate00/01/02 and provisional Gate03 component pass with 1716 host/doc tests,
all 731 M013 controls and 32 M014 controls. Formatting, strict runtime/host Clippy,
architecture and first/final manifest audits pass. The restricted unchanged
empty-derive interpreter/native reference was rerun. The exercised subset retains
43 component assertions. Seventeen unchanged rustc cases and 37 copied files retain
their original licenses/directives; the component harness uses LRR2024. Full
compiletest/native/LRR fixture acceptance and guest JIT acceptance are incomplete.

Next work must combine these captures with the existing hygiene resolver,
explicit expansion ancestry and discovery/publication machinery. Recursive
adoption, transformed namespaces, intrinsic summaries, complete graph/facade impl
seals and reached selection remain pending. M014 and the full project goal remain
active; this capture layer does not replace MX029/MX030 or release acceptance.

See [the checkpoint](../artifacts/macos-first/m014-generated-captures-checkpoint.json).
