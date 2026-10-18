# Actual constant owner Type-prefix prerequisite (read-only V32 diagnosis)

T032/T054/M018/M025; E-R5-005 and E-R5-007. This is a bounded proposed common source-name repair, not implemented or accepted. Initializer-owner-template helper/query53 remain separately source-only. No Rust source changed for this diagnosis.

## Actual witness and fault

Unchanged `tests/ui/associated-consts/associated-const-use-impl-of-same-trait.rs`, owning edition2015/run-pass, reaches `impl Foo for IsFoo2 { const BAR:u32=<IsFoo1 as Foo>::BAR; }`. Exact V32 capture `artifacts/validation-resume-2026-10-18/candidate-v32/stock-6.stdout` is failing with `associated constant qualifier requires its actual source Type application`, original378..398, PC a28defae5b8ff20d62d421698b05b4de8850fa2fafa7766d5cac839c996bdf5a, initializer body7d9c9b0c9794f0b3d397cb110f15dc702435ca794d7a8ff3112a0a31064bbae2 /59724516a194ca4a02c53f0a1f8dea8e243922cbe917259c55244e1751bf0d64, zero source calls/17dispatches/14ops. The independently selected anchored-resolution-order original passes on the same sealed V32 CLI. This distinguishes declaration-owned initializer Type preparation from anchored Types in a function body. The runtime diagnostic does not itself print the internal typed Type demand; the intermediate demand below is established by source control-flow, not a newly executed query.

## Concrete missing common join

`LocalTypeNames::prepare` in `frontend/local_type_names.rs` captures actual declaration AST/header/name/hygiene. It initializes `need_owner = !source.header().context.is_empty()` but only captures/discharges inherited owner bindings under `ast::Fn::cast(declaration.clone())`. An associated `Const` has a genuine nonempty impl header context, so it retains `need_owner=true` even when the actual immediate impl has no generics. In `source_locals::lookup_prefix_inner`, after genuine lexical-item and generic lookup, `evidence.needs_owner()` returns `PrefixResolution::NeedOwnerGeneric` before the ordinary module tail. `source_local_plan::select_type_prefix` preserves `NeedOwnerGenericOccurrence`; the common named Type finalizer preserves typed Pending. `associated_constant_applications::prepare_named_qualifier` correctly does not turn that unresolved demand into a concrete Type. Thus both genuine anchor leaves IsFoo1 and Foo can be blocked before reaching their actual module Type buckets. No need to weaken the Pending boundary or special-case an anchor/constant/type name.

## Proposed bounded source change after writer release

Prefer **one production leaf**, `frontend/local_type_names.rs`, reusing existing canonical binder APIs unchanged. Generalize its immediate associated-owner path to actual Fn/Const/Static declaration ASTs. Keep Fn's current `capture_generics` + `owner_generics` authority; for Const/Static call existing `ParsedFile::initializer_parameters(source.header(),source.context(),source.body())`, which validates the exact actual declaration/body/header and invokes the same `declaration_generics -> associated_owner_generics` kernel used by the real FunctionSignature. Associated constants have no own Type parameter list; do not borrow an enclosing function binder or create a synthetic Fn. Use the actual immediate AssocItemList->Impl/Trait parent and its real GenericParamList. Preserve current canonical own-first/inherited parameter ordinals and `owner_identifier_context` + owning token edition for every inherited Type name. Clear `need_owner` only after actual associated-owner proof and complete inherited-name capture, including the genuinely empty nongeneric impl list. If the retained source lacks the actual parent/header/name/hygiene, preserve Pending. Confirm actual owner-header envelope equals retained HeaderRecipe.context before admitting inherited evidence; existing shared header/context validation must be reused, not parsed from text.

The ordinary LocalScopes traversal, module observation and TypeSchema/trait selection remain unchanged. The actual IsFoo1 leaf then follows the same ModuleTail -> CurrentModuleName -> BodyTypeDeclaration route as it does in main; Foo remains a positively selected trait TypeRecipe with actual trait member identity. No module root guessing, unchecked ownerNone fallback, runtime TypeId reconstruction or alternate qualifier selector. Full generic initializer packet/currentness/cache integration stays explicitly held; this repairs source lookup for real declared owners without discharging that runtime prerequisite.

## Boundaries and cost

Shared source identity/hygiene: actual admitted body/ParsedFile/declaration/header and owner token contexts. Syntax/index: existing BodyTypeOccurrences, actual PathType child and LocalTypePrefix. Constraints/member: same prepared Type prefix, relevant CurrentModuleName bucket and SourceType finalizer. Lowering/VM/cleanup: no change; once source selection succeeds, ordinary associated-value selector and initializer VM/cache remain their existing owners. Original/generated syntax uses the same canonical actual declaration/owner capture; source/epoch/currentness guards remain retained through SourceLocalPlan. No source-only or runtime feature completion claim.

Preparation demand is cold LocalTypeNames construction inside the existing memoized SourceLocalPlan. Cost adds actual immediate owner's generic-name capture/verification for initializer bodies, as ordinary function owners already do. Existing tokens(header) can traverse excluded subtrees before filtering; this is not zero-body-enumeration or constant-time work. No added dispatch/call-entry parse, full-world namespace scan, whole-file search or immutable body copy. The change removes the Fn-only admission restriction at the existing source service; it adds no new lookup path. The obsolete Fn-only condition is removed only at this shared block, not bypassed by a consumer-specific exception.

## Required evidence / all eight review obligations

1. No assertion/expression evaluator: only genuine owner name facts, common source selection unchanged.
2. No scalar/break macro executor: same actual retained/generated Type occurrence and owner AST.
3. No context-specific numeric/member rule: no spelling-specific behavior; actual canonical binders and buckets.
4. Generated syntax stays admitted only with its genuine current owner/name context; no original-only exception.
5. No pattern/eager-arm change: no body lowering or executable branch traversal introduced beyond inherited header token traversal cost disclosed above.
6. Same memoized LocalTypeNames once per source plan; no per-phase/installed re-planning.
7. Same relevant positive/negative bucket dependencies; no world scan or unrelated invalidation.
8. Shared retained ParsedFile/AST/body handles; direct actual declaration locators, no deep clone or repeated complete-file search.

After implementation require exact-leaf source review and renewed independent review, then sole validator's actual unchanged same-trait witness plus existing anchored-resolution-order/Self source controls. Current prior failing capture stays attached to V32; no pass inferred from this diagnosis. Generic initializer packet, Const/Region substitutions, provider applicability publication and once-per-application cache remain separate root-held obligations.

## Source snapshot

- `crates/lrr/src/frontend/local_type_names.rs` SHA256 `59bc96867eb1fce8274627d7c7cb1c6233c8f18f6b7e1c9abd51037d3547df3a`
- `crates/lrr/src/frontend/source_locals.rs` SHA256 `c9bf1a48f19c4f853e5bf2fb59471bb5548c749a25b3659787e4cb536f3993da`
- `crates/lrr/src/frontend/source_local_plan.rs` SHA256 `74d1f18d56fa92af120321739e91a4037abd35b7c385d6f509076df030d716b5`
- `crates/lrr/src/frontend/associated_constant_applications.rs` SHA256 `be6035c7c743bad63f2c740c87797dd578d2849de1e0c3c0093c7f93a1c496bf`
- `crates/lrr/src/syntax/signatures.rs` SHA256 `0112cb3510ff714dbb92e8fd424e064ea298373f947bdd5eee5eae0ddddbfcde`
- `fixtures/upstream/rustc-checkout/tests/ui/associated-consts/associated-const-use-impl-of-same-trait.rs` SHA256 `7c4a55f0c3e5d0b1e507c3daf2edbbce8c708786ab547059372ca9183e933161`
