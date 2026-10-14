# M007 function entry from original containers

ContainerCatalogue now keeps a private entered lexical index alongside its exact
ContainerKey. Explicit function entry validates the source selection, container
revision and declared function body handle before discovering that body. Shared
child-entry machinery captures the parent textual macro frame at the declaration
position, preserves producer ancestry and limits the combined declaration chain.
Recursive module entry therefore retains the macro history at each module and
function declaration. The internal container owner key is never exposed as an
executable body handle.

Module entry starts a fresh import namespace while retaining textual macros.
Trait and impl containers retain their enclosing lexical import context; functions
inside a module inherit that module's own imports. The existing immutable scope
and import view carries each frozen alias's original declaration context through
later body entry. Caller module paths advance at module entry. Executable bodies
stay opaque to the parent, and prior item-macro frontiers remain NeedInput with
their original producer identity even when a later macro declaration is known.
Calls directly in module, trait, impl and extern item lists all register these
declaration-producing frontiers; initializer calls retain expression semantics.

Four source checks exercise module/trait/impl scope capture, recursive modules,
module import boundaries, changed source/container/body revisions, declaration
membership, transforming function rejection and producer frontiers. An actual
macOS Rust1.99 program executes seven calls. Each selected definition is expanded
by the pinned matcher/transcriber from the full original definition forest, and
all seven values match native execution. A separate native witness rejects an
enclosing body alias inside a module. This proves this selected scope behavior;
there is no guest VM or full diagnostic-equivalence claim.

Linked external-module wrapper integration now has separate acceptance
(see linked-container-entry.md). Body-local
module namespace publication, generated publication, local-binding ribs,
namespace seals, guest VM integration and request-wide M012 accounting remain
pending. M007 and release remain incomplete. Actual evidence is recorded in
artifacts/macos-first/m007-container-body-checkpoint.json.
