# M013 captured extern-crate macro-use preludes

M013 remains in progress. Extern macro-use attributes have a distinct stable tag10
and owned All/Only selector recipes. Captured declarations retain checked selector
metadata and source-order ranks. Selectors use pinned identifier tokens and edition
normalization. Extern macro-use requires a recorded dependency at the crate root;
private/unexported named selectors and nested declarations are refused.

The macro namespace now has a bounded crate-wide macro-use prelude. It is available
before its declaration and in nested modules, after textual and explicit path
bindings and before synthetic runtime preludes. The owning unit and actual exported
macro definition are retained, including dollar-crate identity. Selective and empty
lists do not fabricate extra bindings; the prelude creates no qualified consumer
exports. Source no-implicit-prelude policy is carried into discovery routes.

Pending provider export producers are retained as typed demands. Selective imports
wait for generated exports and unknown producers cannot establish export absence or
certify a seal. Completed index and generated route construction use actual remaining
producer sets. Generated directives retain bounded lexical
order paths through their parent fragments. Ordinary imports use last-import order.
Native evidence exposed the additional rule that generated macro-use imports may
not shadow existing macro-use prelude bindings. Textual and path macros retain
their existing precedence; the graph checks earlier prelude imports before publication; initial incorrect native expectations are retained.

Twenty-two controls cover all/selective imports, before-declaration and inherited
access, original/generated/nested output, private/unknown export handling, disabled
and empty preludes, no qualified leakage, textual/named import priority, dollar-crate
identity, generated-shadow refusal and source-order import priority. Provider edits
invalidate old ready seals; stale publication is refused without a database revision
change and the new macro body executes in that same database.

Fourteen actual VM executions match isolated native Rust1.99.0 macOS ARM64 programs.
Six native refusal witnesses cover unselected/private/disabled/qualified/wrong-scope
cases and generated prelude shadowing with a second provider. Oracle compilations are
explicit test preparation only. No LRR query executes a build tool, no selected live
runtime unit is compiled by LRR, and no arbitrary native symbol is loaded.

Rust2015 explicit/prelude rules, anonymous extern declarations, complete procedural
and native macro facets, complete origin overlap/invalidation and generated module
capture, full quota/failure boundaries, runtime/type integration, complete MX027/MX028
and JIT acceptance, M014–M016 and release acceptance remain mandatory. Linux remains
a later port. This is component evidence; no full fixture or release claim is made.
