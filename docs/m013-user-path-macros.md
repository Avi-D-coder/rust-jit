# M013 declarative string operands for compiler inputs

M013 remains in progress. Nested resolution now binds owned user definitions to
Declaration expression keys, preserving definition/input/context/edition and frozen
environment identity. Compiler definitions retain Builtin keys. A user definition
never becomes a compiler intrinsic because it shares concat/include spelling.

The bounded eager worklist requests an external owned call for an unadopted user
string operand. Discovery re-polls and checks the exact current key, definition,
site and token payload before running the pinned declarative adapter outside queries.
Generated local/ancestor bodies use their actual retained generated catalogs;
captured definitions use their exact owning source provider. The coordinator starts,
accepts and adopts the immutable reply separately. The driver consumes only matching
adopted records before resuming concat/include and requesting source capture.
Procedural execution is not authorized by this path. Include views retain the user
producer key/output among their revalidated dependencies, and discovery seals include
those compiler-input stamps.

Ten new controls cover direct literal path macros, user operands of compiler concat,
generated local definitions, renamed source definitions, invalid output, symlink
escape, exact-demand rejection, no self-adoption, explicit reply adoption and seal
membership. Five actual CLI values compare with isolated native macOS ARM64 Rust1.99
copies. The earlier included user-concat refusal control is migrated to positive
admission: its exact original program now succeeds through explicit expansion and
adoption. Existing pure resolver/driver controls now require Declaration keys and
owned external demands while asserting that no expansion record is auto-published.

A native oracle exposed an additional gap: importing a user macro named path can
collide with the compiler's built-in path attribute (E0659), while LRR currently
accepts that import. The positive alias fixture uses the unambiguous name location.
The original source, accepted LRR report and native diagnostic are retained in
artifacts/macos-first/m013-user-path-attribute-collision-witness.json, with
comparison_passed=false and status=pending. This is not a passed refusal fixture or
a claim of complete SDK macro/attribute namespace coverage.

Recursive user expansion results that themselves contain macro/string expressions,
additional SDK attribute/name facets, procedural string operands, general cross-unit/
world/edition paths, multiple source/build-output roots, full Cargo/environment/facet
preparation, compiler-expression runtime lowering, remaining module/origin/quota/
runtime/type integration, M014–M016, full MX027/MX028/JIT and release acceptance remain
mandatory. Linux remains a later port. Component evidence does not complete M013 or
release acceptance.
