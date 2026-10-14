# T054 / WT10-02 imported module route ambiguity — frozen source-only attachment

Preserved separately from sol-constant-foreign-path-selection-v1 and all macro packets. Root reported actual library-v17:759pass/6fail; unchanged issue-25396 path adapter fails at strict ModulePaths::alias conflict before WOMP ambiguity. This packet owns names.rs/module_paths.rs only, two new child acceptance files and explicit cfg(test)/path attachments. No canonical/MAIN/DB/query/evaluator/runtime/macro/generated source/targets/Cargo/compiler/guest actions.

## Exact fix

ModulePaths::alias, declare and explicit_extern_alias remain strict; compiler/direct/extern provenance conflicts are not converted into ordinary source ambiguity. New crate-private imported_alias is used only by the existing ordinary named-import lane after real target module selection and reexport-width validation. Its separate ImportedAliases rows retain each exact target route, VisibilityScope and original owning ScopeKey. Identical route/visibility/origin is idempotent; same target with changed visibility remains a precise conflict. The collector checks counts and retained bytes before mutating the binding and reserves the vector before commit. Bound100k imported routes/32MiB plus existing1M headers/256 path depth.

During iterative module walking, multiple caller-visible imported routes return ModuleResolution::Ambiguous containing their original routes; no first route or widest visibility is chosen. A single caller-visible route continues through the unchanged origin/private-target/reexport-width/active-cycle checks. This does not flatten targets to a guessed canonical member or skip marks.

NameNamespaces now stores all ordinary named module routes in a bounded named_module_links multimap rather than overwriting the one module_aliases destination. Existing compiler/extern module_aliases remains unchanged. Producer-frontier traversal follows the compiler route plus every actual ordinary route, so a genuine unknown producer on either source route remains NeedInput before ambiguity is settled. Deterministic import convergence encoding advances lrr/v3/ordinary-import-convergence→v4 to include all retained routes.

The existing destination-lookup already reports collisions as ambiguity after individually selected targets. Only that settled destination ambiguity with no pending producer now discharges its import frontier; unresolved target ambiguity, missing input and pending producers retain their old frontier behavior. This permits the genuine WOMP lookup to report two original Value identities rather than treating a completed collision as an unknown producer. Source discovery can still report its precise ambiguous/incomplete outcome; no seal or full source completeness is fabricated.

## Apply/integration

production.patch changes only names.rs/module_paths.rs. acceptance.patch adds names_import_ambiguity_tests.rs/module_paths_import_tests.rs. attachment.patch adds explicit #[path] child hooks; omitting it means zero new controls. complete.patch combines the three alternatives. Original issue-25396.rs/.stderr and licenses are pristine and represented in originals.patch/upstream-provenance.json; use the unchanged importer against MAIN for manifest integration rather than overwriting existing pristine originals. Parent owns claim/ledger and actual asynchronous validation.

Eleven controls are uncompiled/unexecuted: unchanged issue-25396 all4 editions exact WOMP/baz/Quux/blah actual source identities; precise competing qualified prefix; deterministic/idempotent source resolution; actual unknown producer on either route; unresolved source imports remain pending; private registry target/origin/idempotence; strict compiler/direct provenance refusals; caller privacy; original target reexport-width refusal; byte/count refusal with no installed row; changed same-route visibility refusal. No compiler static diagnostic/full fixture/runtime contract pass is claimed.

## Preserved limitations

This is an ordinary named-import slice. Declared/compiler aliases remain strict, including different direct-declaration provenance at the same module path. It does not admit general generated/local/associated paths. An ambiguous explicit module member reached while enumerating a glob export remains the existing precise `ambiguous explicit module member` prerequisite; this packet does not invent flattened glob targets or claim broad glob closure support. Multiple distinct source routes to the same final module may conservatively remain ambiguous until genuine route equivalence is proven. Compiler/custom/generated/native catalogue joins remain their actual independent prerequisites. No provider or future body/thunk work occurs.

## Static evidence only

- Existing importer9b6c20/exit0:1case/5files at ae0fe511b15ab760d7e963b13389d1bc2ab42b18, unchanged expected stderr and both licenses+COPYRIGHT.
- Pinned1.99 rustfmt--check e7a7f6/exit0:four explicit files, skip_children.
- Exact private-base git apply--check48a5a0/exit0.

Actual compilation, focused controls, existing unfiltered names/module-path/source-admission banks, strict checks and milestone remain pending. Root canonical f75814bf validation source stays unchanged. Baseline hashes identify the exact privately copied existing files; no passing CA/T/H/M or compatibility claim.
