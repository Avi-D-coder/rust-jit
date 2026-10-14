# M013 completed generated macro import/export routes

Completed root expansion catalogues now contribute generated macro imports and
exports to the existing SourceMacroRoutes builder. The builder reads the generated
catalogue's actual hygienic declaration resolver; it does not manufacture an original
source index from generated text. Textual lookups use captured owner scope, explicit
expansion origin, checked item-name context and the original invocation position.
The existing ancestor resolver remains shared with generated-call selection.

Generated import leaves map to the canonical headers already admitted by
GeneratedHeaderPlan. Scope, path, alias, visibility and grammar must agree before
canonical identity substitution. Generated macro exports also retain canonical
GeneratedItemKey identities and their captured definition edition. Import-only
completed catalogues are retained alongside catalogues that contribute macros.
Existing qualified/alias lookup and exact macro import-facet checks then schedule
later original invocations. Selected macro bodies remain owned captured input;
expansion is an explicit request action, never a provider side effect.

Route construction and retained generated catalogue, textual scope, declaration,
hygiene and ancestry metadata use checked accounting. Source and child catalogue
admission charge their retained metadata before graph publication. This does not
claim complete boundary coverage for every discovery quota.

Seven controls execute the MX027-shaped root scenario: a macro emits another macro
and an import, and a later original invocation produces a function that the VM runs.
They also cover qualified generated exports, imports of captured original exports,
canonical request identity and both record dependencies, cfg-disabled imports,
unresolved generated imports and destination ambiguity. The initial three missing
route failures and a mechanical helper-extraction compile failure are preserved.

M013 remains in progress. Child expansion import/export routes, qualified calls
inside generated fragments, external module/dependency/definition-unit chains,
complete macro namespace query certificates, generated-body runtime macro contexts,
overlapping origins and remaining quota/failure boundaries still require integration.
Generated return/generic aliases and symbolic type paths remain pending. Full
MX027/MX028 CLI fixtures, later M014–M016, full E24, release and Linux acceptance
are not claimed. Validation targets macOS ARM64/Rust 1.99.0.
