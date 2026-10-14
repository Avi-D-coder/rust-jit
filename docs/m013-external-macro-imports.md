# M013 inherited macro imports in captured external modules

M013 remains in progress. Original external-module import routes now use the same
captured parent lookup as direct item calls. The existing SourceMacroRoutes builder
retains the actual selected definition identity under its imported alias; the
existing owner catalogue supplies its captured matcher/transcriber body.

Parent positions remain frozen at actual module declarations. Parent indexes are
read after preceding expansions complete. Nearest-parent and child-local precedence
remain governed by the existing textual/hygienic resolver. No filesystem lookup,
compilation, procedural macro execution or query-provider side effect is added.

Five new controls execute imported parent macros, recursive nearest-parent bindings,
completed generated parent macros and child-local shadowing in the tagged VM.
A future-parent import remains incomplete and cannot publish a seal. The preserved
baseline has two genuine unresolved-alias failures before this integration.

Generated-fragment external parent/import contexts and external owner routing for
generated calls remain pending, as do macro namespace query certificates,
dependency/definition-unit routes, runtime contexts, type/origin/quota boundaries,
full MX027/MX028 CLI fixtures and release acceptance. Candidate import routes are
not negative query certificates. macOS ARM64 Rust 1.99.0 is tested first; Linux
acceptance remains pending.
