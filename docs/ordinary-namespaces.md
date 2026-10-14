# Ordinary namespaces — T020 in progress

NameNamespaces uses separate value/type/macro views of the existing deterministic
owned named/glob import engine. Names and visibility use the pinned source
normalizer and existing visibility ranges. Lookup returns namespace-qualified
source definition identity, ambiguity, absence, inaccessible candidates or a
producer demand. It does not type-check, expand or execute a body.

The first checks exercise equal spellings in three namespaces, explicit import
precedence over conflicting globs, complete empty import cycles versus blocked
cycles, visibility and missing members. A standalone Rust 1.99.0 macOS oracle
compares namespace separation and named-over-glob behavior. These are supplied
owned graph facts, not completed source scope seals.

Block-local/prelude lookup, source/cfg/import publication, module/dependency
routing, unknown producers and aggregate resource matrices remain pending.
T020 is not complete, and no guest execution or complete import/discovery claim
is made by this checkpoint.

Source-outline construction now consumes the pinned outline's active scope
membership and header identities. Tuple constructors retain the same source
identity in value and type views. Modules and visibility come from source facts;
cfg-disabled items are absent, and cold invalid function bodies remain outside
semantic preparation. The constructor rejects missing unit-root outlines and
unknown candidate/header references and performs checked staging accounting.

Scoped unknown producers are retained at their original scope. Lookup checks
relevant module prefixes before returning a disclosed definition or absence,
while unrelated complete siblings remain available. Unresolved ordinary imports
remain source frontiers until import integration resolves them; they are not
silently dropped. This first source view does not provide complete ordinary
import resolution, lexical/prelude lookup or a catalog seal. Large aggregate
boundary matrices and full T020 acceptance remain pending.

Named source imports now install symbolic edges in each namespace, preserving
original definition identity across grouped aliases and public alias chains.
Each grouped leaf has a stable identity combining statement identity and leaf
ordinal. Resolution reports resolved, pending, missing, inaccessible or ambiguous
leaves; only resolved leaves discharge their producer frontier. Intermediate
alias visibility is checked before discharge. Native Rust rejects widening a
private alias via a public re-export (E0364), agreeing with the existing engine;
the initially invalid positive fixture was corrected and retained as a negative
regression. Unsupported module aliases, globs and anonymous imports remain
pending, including named consumers that depend on them. Complete source imports
and scope seals are still pending.

Disclosed module aliases now join named imports through a bounded deterministic
worklist and the existing module registry in all namespaces. Targets keep their
original paths, preserving intermediate visibility checks. Module aliases and
member imports preserve source identity through chains; alias-prefix frontier
traversal retains unknown producers at the original module. An invalid public
module re-export reports inaccessible rather than falsely blocking resolution
of a valid sibling. The actual macOS native module-alias/member comparison agrees.
Globs, anonymous/root/dependency aliases, lexical/prelude lookup, aggregate limits
and complete scope acceptance remain pending; T020 is still in progress.

Source glob member scheduling now installs namespace-qualified weak edges over
the finite disclosed member-name universe in the existing import engine.
Explicit imports win; weak ambiguity remains distinct; inaccessible private
members are filtered. Disclosed empty and populated glob cycles can close when
all reachable frontiers are discharged or belong to the disclosed glob graph.
Unknown producers keep disclosed and missing members pending, and defer missing
module diagnostics. A native macOS populated-cycle oracle agrees on member
identity. Edges and metadata use checked limits. Glob-imported module prefixes,
full strong/glob module selection, anonymous/root/dependency aliases, lexical
and prelude lookup, aggregate acceptance, and complete T020 remain pending.

Disclosed glob module members now populate the existing module registry with
intersected source/import visibility in all namespaces. Qualified members and
named imports retain original source identity. Multiple weak module origins
return typed module-prefix ambiguity. Explicit modules suppress weak origins
and their descendant frontier traversal; unshadowed unknown descendants still
block disclosed and missing qualified members. The actual native macOS qualified
member comparison passes. This is not full strong/glob module fixed-point
acceptance; anonymous/root/dependency aliases, lexical/prelude lookup, aggregate
matrices and complete T020 remain pending.

Root aliases now retain original module paths and return typed compilation-unit
root identities without inventing source item definitions. Supplied dependency
renames populate the existing dependency/module registries in each namespace.
Absolute dependency root aliases and qualified members preserve provider source
identity and enforce cross-unit privacy. Disclosed local modules shadow extern
prelude names without gaining a shadowed provider's unknown frontiers. Unknown
provider producers still defer disclosed/missing members through actual root
aliases; invalid private root-alias widening is inaccessible. The actual native
two-crate macOS comparison passes. Full project/external module routing,
strong/glob fixed point, anonymous imports, lexical/prelude lookup, aggregate
acceptance, and T020 completion remain pending.

Ordinary lexical lookup now joins the exact entered source-local index to the
ordinary namespace view. Value locals return their original binding identities
before module lookup; unresolved local producers suspend fallback. Source/body,
position and pinned hygiene checks remain those of the existing local index.
Module names precede an explicit supplied prelude scope. Namespace separation
and absence of implicit parent-module lexical capture are tested. A native
macOS oracle agrees on module, outer-local, inner-local and restored outer-local
selection; stale source/body and out-of-range positions refuse. Body-local
item/import ribs, complete prelude/profile integration, exact ordinary namespace/
source lease pairing, full strong/glob import fixed point, aggregate acceptance,
and T020 completion remain pending.

Ordinary body lookup now requires the namespace view to retain the exact source
outline key and consumed namespace-input fingerprint of the entered source.
Content/profile/cfg/unit/module, header/member facts and original imports are
covered; original imports use explicit domain-separated stable encoding.
Stale or unbound graphs, wrong cfg/unit/profile and nine import-field mutations
under an unchanged claimed content key refuse. Proof staging metadata uses
checked bounds. This proof pairs source revisions; it does not establish scope
completeness or certify generated facts. Body-local item/import ribs, full
prelude/profile and linked/generated integrations, full strong/glob import
fixed point, aggregate acceptance and T020 completion remain pending.

Original entered block items now join ordinary lookup with exact source identity
and block ancestry. The hygienic local walk reports the selected scope, allowing
inner items to shadow outer locals while visible same-scope locals win. Item
headers are available throughout their block; constructor facets remain distinct.
Unknown item/attribute producers and unresolved imports retain hoisted frontiers,
including at earlier source positions. Macro lookup delegates to the existing
textual resolver, preserving macro_rules definition order. Metadata and ancestry
use checked bounds. A native macOS hoisted-item comparison passes. Oracle output
macros are in a separate block; same-block unknown producer suspension remains
a dedicated regression. Body imports, nested-function item captures, generated
item ribs, full prelude/profile integration, the strong/glob fixed point,
aggregate acceptance and T020 completion remain pending.

Nested entered functions now retain enclosing original block-item headers and
producer frontiers from the exact scope chain, with remapped namespace ordinals
and unchanged source definition identities. Ordinary lookup follows captured
item ancestry while enclosing value bindings remain outside the nested local
index. Two function levels preserve the enclosing helper identity; enclosing
unknown producers suspend lookup and unrelated sibling scopes stay separate.
Metadata and ancestry retain checked bounds; macros keep the separate textual
resolver. An actual native macOS two-level comparison passes. Body import
resolution, generated items, full prelude/profile/linked integration, the
strong/glob fixed point, aggregate acceptance and T020 completion remain pending.

Body-local named imports now use hoisted lexical alias indexes and the existing
ordinary module target resolver. Bare alias chains and direct qualified source
paths preserve original source definition/unit/namespace identity, including
captured enclosing imports in nested functions. Type-only imports do not create
value bindings. Scope import readiness retains unresolved targets, cycles and
producer demands as typed pending frontiers; anonymous/glob/attributed imports
remain frontiers. Work and scratch metadata use checked admission, with aggregate
boundary acceptance still pending. An actual native macOS captured alias-chain
comparison passes. Body globs/module aliases, generated imports/items, final
import diagnostics, full prelude/profile/linked integration, the strong/glob
fixed point, aggregate acceptance and T020 completion remain pending.

Qualified body import routes now traverse hoisted module/root aliases and
captured enclosing alias chains. Routes retain original import targets and
declaring source scopes before final selection by the existing module resolver.
Work, ancestry, path depth and scratch metadata use checked limits. Cycles and
unknown targets cannot supply members; supported members keep original source
identity. An actual native macOS captured root/module import-chain comparison
passes. This is qualified import routing: direct body-qualified expression paths,
body globs, generated imports/items, final diagnostics, full prelude/profile/
linked integration, the strong/glob fixed point, aggregate acceptance and T020
completion remain pending.

Direct qualified module lookup in an exact entered body now reuses body
module/root alias routing. Body/source/context/position and namespace pairing
validate before selection. Path demands use an explicit stable domain with
retained body, source point, namespace and context. Qualified member identity
survives unrelated local spelling collisions; original private visibility and
unknown producer frontiers remain enforced. Empty and stale sites refuse.
A native macOS direct qualified-member comparison passes. Associated paths
belong to the later type/trait resolver; qualified macros retain invocation
lookup. Body globs, generated imports/items, final diagnostics, full prelude/
profile/linked integration, the strong/glob fixed point, aggregate acceptance
and T020 completion remain pending.

Body-local glob member demands now use the existing ordinary module and
visibility resolver through body alias routes. Weak ambiguity and private-member
filtering remain explicit; strong items/imports win. Named aliases can select
glob members through enclosing captures in nested functions. Unknown producers
keep disclosed and missing members pending. Resolving a strong item does not
seal an open glob or establish scope completeness. Checked work/candidate
metadata is implemented; aggregate boundary acceptance remains pending. A native
macOS captured glob/alias comparison passes. Body glob module-prefix/fixed-point
routing, generated imports/items, final diagnostics, complete prelude/profile/
linked integration, the global strong/glob fixed point, aggregate acceptance and
T020 completion remain pending.

Tuple constructor value visibility now intersects the struct visibility with
cfg-selected original field visibility from the pinned AST. The type facet
retains the struct's visibility. Field ranges are owned item facts, explicitly
encoded and included in metadata accounting and nested captures. Named imports
can select the accessible type facet without granting the private constructor.
The initial incorrect public constructor lookup is retained as a failing log.
Native macOS accepts public constructors, cfg-disabled private fields and public
type imports; it rejects the private constructor with E0603. This does not claim
field-type checking, VM execution or generated-field integration. Remaining
body/global fixed points, generated facts, diagnostics, full prelude/profile/
linked integration, aggregate acceptance and T020 completion are still pending.

Constructor facets now have body alias/glob/nested-capture acceptance coverage.
Public types remain selectable; private value constructors remain inaccessible
through explicit aliases and absent from glob value facets. Public constructors
stay selectable. The existing production path satisfies these cases without
another behavior change. Native macOS captured type/public-constructor execution
passes and private alias construction rejects with E0423. VM/type checking and
generated-field completion are not claimed. The remaining body/global fixed
points, generated facts, diagnostics, full prelude/profile/linked integration,
aggregate acceptance and T020 completion remain pending.

Disclosed body-glob module prefixes now feed direct qualified members and named
member imports through original source scopes and the existing module registry.
Explicit named/declared type prefixes suppress weak module origins. Prefix
ambiguity precedes member selection even when only one candidate module has the
leaf. Native macOS agrees on the resolved member and rejects the ambiguous
prefix with E0659. Named glob-base aliases expand through bounded named routing;
full chained-glob/strong-glob fixed-point acceptance is not claimed. Generated
facts, final diagnostics, complete prelude/profile/linked integration,
aggregate acceptance and T020 completion also remain pending.

Body-import readiness now reuses resolved single-path alias chains within one
immutable ordinary lookup. Memo keys include the starting body scope, namespace,
and normalized name. Only Defined, ModuleRoot, and Absent outcomes from a traversal
without branches or cycles may populate intermediate aliases; pending, private,
and ambiguous results retain their existing paths. Memo storage has a checked
32 MiB metadata limit and does not survive the call or execute tools.

The 1,001-alias acceptance regression previously failed with the named-import
work-limit diagnostic and now resolves the original function identity. The exact
source also compiles and executes under native macOS Rust 1.99.0. This is work
budget acceptance, not a runtime performance benchmark. Full chained strong/glob
closure, generated facts, final diagnostics, full prelude/profile/linked
integration, aggregate acceptance, and T020 completion remain pending.

Body-glob bases can now resolve through other body globs using an explicit
continuation stack. Named aliases expand with their declaring body scope; each
weak candidate's base is resolved before its requested module prefix is tested
against the owned module registry. Explicit type prefixes stop weak search.
Active glob identities cut cycles, and returned routes retain original source
scope and visibility checks. Traversal shares the one-million work limit, caps
active depth/ancestry at 256, and checks continuation/route metadata at 32 MiB.

The chained-base fixture selects the original function through both a named
member import and an unqualified glob member. Controls retain ambiguity before
leaf selection, explicit prefix precedence, pending source producers, and
unanchored glob targets. Native macOS accepts the chain and explicit
prefix cases and rejects the ambiguous prefix with E0659. This closes the
previous one-layer body-glob routing limit; it does not establish complete
global strong/glob convergence, generated facts, final diagnostics, complete
prelude/profile/linked integration, aggregate boundaries, or T020 completion.

Global source imports now run bounded deterministic resolution rounds until an
explicitly encoded state fingerprint stabilizes. Each round re-establishes the
original import demands, schedules named aliases, computes the next complete
source-derived glob edge set, retracts superseded item edges, and rebuilds weak
module edges. Manual glob item edges retain separate ownership; removal retains
any named/manual edge still referencing the target. Module byte charges and glob
header counts are restored on removal, with checked accounting failures.

These helpers only mutate the caller-owned source index while it is being built;
they do not read files, run tools, execute macros, or publish discovery seals.
The convergence fingerprint uses domain-tagged stable field encodings; it
includes source-derived item/module edges, original module aliases, import
frontiers and deferred imports. Stable pending/ambiguous results remain typed
and do not establish source completeness. The shared convergence work budget
is one million; namespace tables retain their existing size/quota guards.

Acceptance covers a multi-stage global chain, a later explicit prefix that
replaces an earlier weak member origin, retained manual edges, repeated
resolution, module-byte/header-count restoration, late module ambiguity and
open producer demands. Native macOS distinct function bodies confirm that the
later explicit import selects 3 rather than the competing 7. This advances
global import convergence; complete project-unit routing, anonymous imports,
generated facts, final diagnostics, full prelude/profile/linked integration,
aggregate boundaries and T020 completion remain pending.

Anonymous imports now classify source readiness and retain original definition
facets without adding a named edge. Module indexes expose disclosed records;
body indexes validate exact source/body/context, follow named/glob aliases and
captured enclosing scopes, and return a typed pending demand if the requested
record set is blocked. Root imports retain unit identity without inventing a
nominal definition. Records have explicit stable encodings and checked aggregate
metadata limits. Private value constructors do not hide an accessible type facet.

Acceptance covers module/body imports, alias and capture origin, missing/private/
ambiguous/pending outcomes, inactive cfg imports and stale selections. Native
macOS confirms anonymous trait visibility and rejects the private import with
E0603. These are source-facet/readiness checks; LRR trait dispatch and generated
discovery seals remain later integration. See the T020 component acceptance
audit for the requirement-by-requirement boundary and pending full fixtures.
