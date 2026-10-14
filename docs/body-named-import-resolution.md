# M007 original named import routes in entered bodies

Entered invocation lookup now consults original named-import facts after textual
macro selection. Block imports apply throughout their lexical scope; nearest
block bindings shadow containing block imports. Imported paths are rewritten
through forward named aliases and module aliases before using the existing
caller-aware macro/module graph. Trailing self-as-module import syntax is
preserved by the syntax inventory and consumed as a module alias route.

Per-walk state tracks lexical scope, qualification and owned path components.
Repeated complete alias states settle Absent rather than caching an incomplete
answer. Multiple strong import bindings fail with sorted source identities.
Active transforming import facts preserve NeedInput. Original body glob
contributions from disclosed modules now integrate with these routes; see
`docs/body-glob-import-resolution.md`. Body module membership from original imports now resolves through the
synchronous route table in `docs/body-glob-module-membership.md`; body item
catalogues and namespace seals remain pending.
Checked work, route depth, metadata and allocation admission bound the walk.
No namespace binding is published into an unrelated module or body.

Two source acceptance checks cover scope-wide visibility before the use item,
nested shadowing, forward aliases, module aliases and complete empty cycles.
An actual macOS Rust1.99 native fixture compares selected original exported macro
identities using pinned adapter hygiene and a parsed source graph, and executes
the scope-wide/nested alias program. It records actual native values, without
claiming guest VM execution.

Body-local textual-definition targets now resolve at the retained use declaration
start and block scope. Aliases therefore keep the original definition across
later redefinitions and caller-block shadowing; forwarding through another
body alias preserves that alias declaration too. This uses the same lexical
resolver and captured outer scopes. Only private invocation resolution can
consult the retained use-item context: public lookups still reject opaque
syntax and source/body selection mismatches. A source regression and an actual
macOS Rust1.99 native comparison cover these cases.

Body item/module catalogues, additional
ambiguity/cycle/visibility integration, generated publication, source local
binding ribs, membership seals, guest VM and M012 aggregate accounting remain
pending. M007 and the full release remain incomplete.
