# M013 standalone compiler facade routes

M013 remains in progress. Explicit standalone compiler preparation configures core
and std export facets using the existing facade unit identities. The std routes
retain their core compiler definition owner and pinned descriptor identity. Compiler
facets are configured before initial declarative route installation, so explicit
SDK extern declarations are validated during that first scan.

Source route construction distinguishes available dependency providers for explicit
extern declarations from implicit dependency aliases. Existing recorded aliases
retain precedence over synthetic SDK defaults. no_std omits the implicit std alias;
no_implicit_prelude disables inherited implicit lookup. Explicit named extern aliases
remain structural bindings into the available export facets. Ordinary namespace
routes register compiler dependency facts without fabricating strong aliases, so
local modules shadow relative core/std paths and absolute paths use extern lookup.
SDK roots and disabled policies join the existing namespace/import engine. Runtime
macro preparation uses the same explicit-versus-implicit distinction, including std
exports made reachable through an explicit extern declaration under no_std.

Sixteen new controls cover qualified core/std includes, nested qualified concat,
named/glob imports, local shadowing, absolute paths, no_std/no_implicit refusals,
explicit restoration and runtime assert through restored aliases. Nine actual CLI
values match isolated native macOS ARM64 Rust1.99.0 copies. Native refusal witnesses
exercise E0433 direct paths and E0432 imports; initial diagnostic assumptions/failures
remain archived. Capture and output admission still precede discovery completion;
no query filesystem/tool execution or native symbol authority is added.

Complete SDK macro-use inventories, nested user/procedural argument macros, full
Cargo/build-output/environment and per-unit preparation, generated/nested includes,
multiple source roots, world/native/procedural facets, origin/invalidation/quota/
runtime/type requirements, full MX027/MX028/JIT and release acceptance remain pending.
M014–M016 retain their fixed order. Linux remains a later port. This checkpoint is
component evidence and does not mark M013 or release acceptance complete.
