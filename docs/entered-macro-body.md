# Explicit entered-body macro discovery

PublishedMacroIndex::enter_body selects an exact retained BodyKey from its
published source view, checks entry boundaries and source bytes, and returns a
borrowed EnteredMacroIndex. The parent lease remains alive throughout use. Every
lookup checks both source selection and body revision, plus its entered source
position. Function/body keys and local canonical names identify declarations;
upstream arena IDs and byte offsets are not persisted as declaration identity.

The existing pinned AST supplies lexical blocks, macro declarations, cfg and
attribute facets. Traversal uses checked iterative frontiers, allocation and
record/depth limits. Macro token trees, nested functions, closures and other item
interiors stay opaque; lookup into them requires separate entry. Definitions
retain exact source names/spans and known export attributes. The index freezes
parent visibility when a block opens and joins body candidates with the exact
containing source scope through the existing pinned-name/origin selector.

Standalone statement producers return typed input demand and execute nothing.
Expression macro calls inside initializers do not introduce an outward producer
frontier. The parser's shared MACRO_CALL kind is interpreted using its actual
StmtList/ExprStmt position, not Item::cast alone. Generated declaration/origin
publication remains later expansion work. Export facets are retained here;
root export catalog insertion remains pending.

Three source regressions cover block ordering, wrong-body/range rejection,
opaque nested executables, statement suspension, cfg filtering and export
metadata. The native macOS fixture discovers all selected source declarations
from the exact published body, applies transported pinned name facts and expands
selected original source slices with the pinned MBE. Its values match native
[17,17,41,29]. This is lexical source/expansion acceptance, not guest VM execution.
Local binding discovery, nested executable entry, imported/exported catalogs,
generated epochs, complete arena/deadline budgets and later guest integration
remain required. M007 stays in progress.
