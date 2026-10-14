# M013 captured explicit extern-crate macro paths

M013 remains in progress. The pinned AST now captures extern-crate declarations
as owned, bounded, stably encoded structural facts: canonical source identity,
lexical scope,
prepared dependency name, local alias and visibility. These declarations no longer
create missing external-module placeholders. They resolve only recorded preparation
rows (or the explicit self unit); uncaptured units are refused before discovery.
Cfg-disabled declarations contribute no alias.

Macro-route construction seeds explicit module aliases before its named/glob fixed
point and preserves lexical visibility. Public aliases support macro reexports;
private aliases remain inaccessible outside their defining module. Collisions with captured local modules
and duplicate declarations within one captured outline are refused. Generated headers retain
the original or renamed identifier and its pinned span rather than treating an
extern-crate item as an unnamed opaque header. Definition identity and body ownership
continue to resolve the actual captured source unit. Metadata is charged before
capture, route retention and bounded fixed-point/index publication.

Fourteen registered controls exercise root/nested/generated aliases, public reexports,
cfg exclusion, no-implicit-prelude access, uncaptured-unit refusal, private visibility,
self identity, duplicate/module collisions, unresolved replacing-attribute barriers
and explicit anonymous-form refusal. The explicit native oracle compares
seven actual VM values to isolated Rust 1.99.0 macOS ARM64 binaries and requires the
native E0603/E0259/E0260/E0433 refusals. Oracle compilation is a test preparation action;
no selected live runtime unit is compiled by LRR and no query executes a build tool.
Unresolved replacing attributes retain their producer barrier and publish no alias.
Initial failing regression logs and intermediate diagnostic corrections are retained.

This checkpoint covers qualified macro paths in the live 2024 profile. Rust2015
explicit extern/prelude rules, anonymous extern declarations, extern-crate macro_use
and selective import forms, cross-fragment alias conflicts, full generated-source
origin overlap/invalidation,
Cargo/world/facets, complete MX027/MX028 and JIT acceptance, runtime/type/quota
integration and M014–M016 remain pending. No full fixture, release or Linux claim.
