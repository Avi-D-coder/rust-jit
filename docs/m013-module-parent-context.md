# M013 generated module parent context

M013 remains in progress. Discovery macro catalogues now retain generated external
modules as explicit typed frontiers. Each frontier keeps the actual generated
module demand, parent lexical scope and declaration position. An unresolved
module prevents outward catalogue completion and makes an otherwise idle
catalogue return NeedInput; it is never certified as an empty child module.
The strict standalone completed-catalogue constructor still refuses unresolved
modules. Module metadata joins the existing checked catalogue quotas.

An external child scope keeps its own unresolved producer. A macro_use export
therefore remains pending while its child is undiscovered. Cfg-disabled modules
create no demand. Parent textual lookup occurs at the actual generated module
header, respecting nested scopes and excluding definitions after that header.
The pinned owned header planner receives the retained caller root explicitly;
nested expansion contexts are not substituted for the original source root.

SourceDiscovery retains the parent fragment when module demands remain, rather
than discarding its macro catalogue. Adopted module parent lookup first validates
the exact frozen child view and re-observes caller/cfg/environment. It joins the
retained generated lexical context with the captured original provider at the
original macro invocation position, using the existing hygiene, origin and
ancestry resolver. It performs no macro execution or source capture.

Three registered controls verify parent textual cutoff and an exact native macOS
Rust1.99 comparison against actual pinned helper expansion, nested shadowing,
cfg-disabled modules, unresolved macro_use imports, repeated suspension, unknown
demand refusal, and adoption-gated joining of generated and original enclosing
contexts. The prior compiler/include/string regression suites also exercise the
retained-root header-planning path.

Child macro calls themselves are not yet scheduled or executed by this lookup.
Complete child declaration/import/macro worklists, derived source publication,
recursive module discovery, included-file physical layout, final frontier
closure and actual LRR child execution remain required. Qualified macro-path
resolution and complete cross-unit/world handling remain part of that integration.
The historical positive generated-module LRR/native witness remains pending with
comparison_passed=false. This native context comparison is component evidence,
not full MX027/MX028, JIT, release or Linux acceptance. The current platform is
macOS ARM64 Rust1.99.0; Linux is deferred.
