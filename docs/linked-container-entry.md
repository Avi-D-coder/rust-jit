# M007 linked original container entry

LinkedEnteredMacroIndex exposes explicit container entry. Its borrowed
LinkedContainerCatalogue wraps the existing original container catalogue and
retains the same LinkedModuleIndex, including every external source ancestor.
Container entry, recursive container entry and function entry validate the entire
current/parent/root source-selection chain before accessing its current element.
Existing exact container/body/declaration/transform guards remain in force.
Functions entered through these containers return the existing linked body index,
so textual lookup and import-target lookup both receive external-module outer
frames as well as captured body/container frames.

A three-file source check covers local and recursive module containers, frozen
imports from ancestor macro definitions, function entry, stale current/child/root
sources, missing ancestors, wrong container revisions and opaque-parent lookup.
An actual Rust1.99 macOS program executes nine calls. The pinned matcher/transcriber
expands every selected original definition from full source forests. Results match
native execution, including a macro defined inside the local container and
captured by its deeper module. Later file/body redefinitions preserve the earlier
declaration captures. This closes this wrapper's previous integration gap; it
does not claim guest execution or body-local global namespace publication.

M007 still needs source local-binding discovery and full source integration.
Generated publication, discovery seals, VM integration and request-wide expansion
accounting remain required later work. Release remains incomplete. Evidence:
artifacts/macos-first/m007-linked-container-checkpoint.json.
