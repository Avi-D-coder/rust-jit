# M013 generated declarative macro component

The pinned item parser retains owned macro body token trees with original spans,
contexts and the edition of the macro_rules keyword. Bodies remain opaque during
item admission and catalog construction. Their cumulative wire size is checked
against 256 MiB. Generated header identities remain tied to the exact expansion
and structural ordinal.

GeneratedMacroCatalog refuses incomplete fragments with unresolved attributes,
derives, item macros or external module sources. It consumes cfg-filtered root
macro headers from the existing owned syntax replay. It validates the explicit caller scope/context and discovery
epoch, registers definitions in TextualScopes and MacroDeclarations, and uses the
pinned adapter's immutable hygiene normal forms and comparison classes. Definition
ownership comes from the caller source scope; token location does not establish
that authority. Positions follow the existing source-header start-offset convention,
including adjacent declarations. Lookup uses the existing hygienic resolver.

Only an explicit expansion demand parses a retained body with the pinned matcher.
The demand checks declaration identity, definition edition and actual input wire
fingerprint and the supplied call-site context/edition, then passes explicit
definition ownership and expansion identity to hygienic transcription. Output limits remain two million tokens and 32 MiB.
Malformed rule syntax is a typed expansion error on demand. This component neither
runs a procedural macro nor invokes build tools or compiles live runtime units.

Six controls exercise actual generated macro output: later generated function
headers and literal tokens, textual shadowing, cfg exclusion and lazy malformed
rules, and refusal of substituted demand identity/input/edition/provenance and
stale or mismatched catalog ownership. Unresolved transforming attributes,
derives, item macros and external modules return a typed incomplete result. These controls
exercise one generated root fragment. They do not certify cross-fragment source
order, enclosing source ancestry, nested scopes, imports of macro exports, transformed
attribute frontiers, query-provider suspension, or complete source discovery seals.
GeneratedHeaderPlan still keeps macro producers incomplete until full discovery
integration discharges them. M013 and MX027/MX028 remain pending. Release and Linux
acceptance remain pending; validation is on macOS ARM64 with Rust 1.99.0.
