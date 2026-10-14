# M007 explicit nested function entry

EnteredMacroIndex now admits a retained nested function through enter_nested_body.
The caller supplies the exact source selection, parent BodyKey and child BodyKey.
The child must be an original function header in the parent's immutable facts;
changed, unrelated, unentered-grandchild and transforming body handles cannot be
used to bypass that boundary. The selected parser checks the exact retained child
range and byte hash before discovering its body. Parent lookup keeps rejecting
calls inside the opaque nested declaration.

Nested textual lookup retains enclosing body frames at the child's declaration
position, followed by the original published/module ancestry. Later parent macro
redefinitions therefore cannot change the child's captured lookup. Producer-kind
metadata borrows the same body ancestry. The linked external-module entry wrapper
revalidates its complete source chain and appends its existing module ancestry;
linked-nested acceptance now passes; see `docs/linked-nested-body-entry.md`.

The child's namespace view owns only scope/import metadata. It retains original
import identities, source module/owner, visibility, raw paths, attributes and
import declaration positions. Virtual lexical ordinals connect the child's root
to the captured enclosing scope chain. Each inherited import also retains its
original textual frame, so a frozen macro alias is resolved at that import's
position rather than at the child's invocation. Existing named/glob/module
resolvers consume the combined view, preserving scope-wide imports, namespace
facets, visibility, identity deduplication and inner-glob/outer-import ambiguity.
No ancestor declaration body is lowered or published into another source module.
Scope depth, metadata, import count and allocations are checked.

Two source acceptance checks cover child/grandchild capture, later redefinition,
inherited frozen aliases, opaque-parent lookup rejection, wrong source/parent/
child revisions, inherited glob ambiguity and transforming entry rejection.
An actual macOS Rust1.99 native program executes the same three captured calls;
pinned hygiene selections are compared with the original source definition
identity. This evidence does not claim guest VM execution.

Container entry and local module/item publication, local binding ribs, generated publication, namespace seals, guest VM
integration and request-wide M012 accounting remain pending. M007 and the release
remain incomplete.
