# M013 qualified generated macro calls

Generated calls now obtain module path facts from their exact canonical header in
the pinned owned syntax tree. Original source and attribute entry points preserve
their existing path restrictions. The generated path entry point permits `$crate`
syntax, while dispatch requires the exact emitting token's definition-site hygiene
authority. No display text is republished as original source.

The dispatcher preserves textual lookup and its pending-producer guard. Qualified
calls and unresolved bare aliases then use the existing macro/module route engine.
Path token item-name contexts must agree. The route catalogue combines captured
source facts, current generated facts at actual ancestor positions and immutable
completed child snapshots. Partial route facts provide candidate selection rather
than negative query certificates. Size, count and allocation limits remain checked.

`$crate` starts from HygieneFacts' explicit definition unit; ordinary `crate`, `self`
and `super` routing uses the captured caller scope. Visibility and ambiguity checks
remain in MacroNamespaces. Selected requests retain the canonical definition identity
and existing body ownership, edition, input-token and context checks. Expansion is
still an explicit request action, never a provider side effect.

Eleven controls execute original and generated qualified exports, an alias imported
inside the same fragment, a nested qualified call, `$crate`, a raw keyword and `super`
from an inline module. They check canonical generated request identity and both record
dependencies, refuse private access and ambiguity, and prevent a seal for an unresolved
path. Seven initial unsupported-route failures and the mechanical syntax API mismatch
are retained. The mismatch was repaired from the pinned dependency source: `$crate`
is represented as a special name reference. Related root and child route controls pass.

M013 remains in progress. External captured module/dependency/definition-unit chains,
complete macro namespace query certificates, generated-body runtime macro contexts,
overlapping origins and remaining quota/failure boundaries require integration.
Generated return/generic aliases and symbolic type paths remain pending. Full
MX027/MX028 CLI fixtures, later M014–M016, full E24, release and Linux acceptance
are not claimed. Validation targets macOS ARM64/Rust 1.99.0.
