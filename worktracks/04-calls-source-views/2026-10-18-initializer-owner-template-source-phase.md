# Code-ready source-phase proposal: actual initializer owner template

Read-only during V32 standing freeze. This narrows the broader item-application packet proposal; no Rust implementation or runtime acceptance is claimed. T032/T054/M018/M025, E-R5-007 and E-R5-005. Use the actual successor first fault before choosing implementation. No new parser, executor or global member inventory.

## Minimal source API

Prepare `InitializerOwnerTemplate(definition)` in the existing pure memo framework, keyed by the selected constant definition and its captured real declaration input dependencies. Its result is **declaration facts**, not a selected applicable provider or permission to execute/cache. Do not put a caller-selected runtime TypeId, entire caller environment or purported provider into that declaration key.

Actual inputs are the existing `FunctionSignature(definition)` result and `DefinitionHeader`, `DefinitionSource`, `DeclarationContext` inputs; `ParseFile(actual ParseKey)` supplies the shared parser. Check signature.definition/header/scope/profile against those inputs. Select the exact Const/Static AST at DeclarationContext.range with the existing direct `syntax_at` locator. Check the actual declaration kind and actual header tokens excluding only its own `body()` expression, following the existing declaration_return_types actual Const/Static header route. Do not require/lower BodyIndex/Thunk/entered BodyEnvironment to answer this header goal; no missing-body input should be introduced merely to prove no owner parameters. Preserve source cfg/edition/generated occurrence authority through the real selected declaration/publication, not an original-only shortcut.

From that real declaration, its immediate `AssocItemList -> Impl/Trait` parent is the source owner. The parent header (excluding its real associated-item list) must match the retained HeaderRecipe.context. The selected constant's real FunctionSignature already carries own-first then owner generic ordinals, and implicit trait Self if applicable. Reuse the canonical signature capture and semantic-name/lifetime helpers rather than inventing a fresh binder order or raw spelling map. A retained fragment lacking its actual owner or canonical publication remains Pending.

Proposed result retains the selected signature query Arc/source/context fingerprint, owner kind/identity, actual owner/declaration ranges or shared AST handles, parameter kind/ordinal map, and actual source owner Self template (optional for an unowned item). An Impl's `self_ty()` is lowered by the existing `WrittenTypeDeclaration::function_syntax`/`resolve_actual_type` walker using the **constant's signature and real Const syntax**. Consequently every `T` in `Proxy<T>` already has `ParamKey { definition: constant, ordinal: actual inherited ordinal }`; no impl-to-const ordinal guess or copied caller keys. A Trait owner identifies the genuine implicit Self ParamKey and actual trait parameters. Unresolved header names/unsupported argument kinds are explicit Pending, not fake Type names.

Do not call/extend the historical `source_types::{impl_self_type,generated_impl_self_type,impl_owned_name}` synthetic-header parsers. This query uses the actual already-retained parent syntax exclusively.

## Exact Proxy input/output

For the real `impl<T: Foo> Foo for Proxy<T> { const X:i32=T::X; }`, source output is owner=that canonical impl, real constant signature parameter `Type T` at its captured ordinal, and `TyTerm::Nominal(ProxyDefinition, [GenericArg::Type(TyTerm::Param(constant,Tordinal))])`. It contains no reached `Abc`/`Def`, no descriptors and no invented closure identity.

A later applied-item goal combines this immutable template with actual reached source Self `Proxy<Abc>` or `Proxy<Def>` from the caller's paired source/runtime binding. The existing Type SignatureSubstitution relation can yield `{T=Abc}` or `{T=Def}` for this Type-only case. Ordinary call contracts receive the resulting `(ordinal, ArgumentType {source, descriptor})`; a separate full canonical `(BoundKind,ParamKey,GenericArg)` application remains the cache identity. Type descriptors are paired physical bindings only.

The ordinary VM Unifier intentionally ignores regions and treats some array consts permissively; its relation is not a full Const/Region identity proof. Keep exact source GenericArg values before runtime relation. A complete application must cover every genuine signature parameter and preserve nested source arguments without unresolved caller parameters, inference/placeholder identities or descriptor-derived substitutes. The present GenericEnv lacks Const/Region bindings, so declarations needing them remain pending until the shared packet/environment prerequisite exists. Wider/signed ConstTerm representation remains held rather than truncated.

## Provider and currentness boundary

Actual member selection remains a **separate prerequisite**. `AssociatedConstSelection` retains the real provider impl and trait/default distinction but currently has no independently captured relevant member bucket/absence proof. A template query cannot make that selection current merely by accepting its digest.

At the integration join, an override's template owner must match selected.implementation(); a provided trait constant's template owner must match selected.trait_definition(), with the actual provider retained separately. Canonical module impl identity is the outline's enclosing_impl identity (possibly published/rebased), not necessarily the impl DefKey fingerprint; body-local and generated identities require their actual canonical publication mapping. Do not recompute a declaration identity from header text or cast a DeclarationScan identity to BodyItem identity.

A real applied-member proof needs positive/negative dependencies for relevant nominal/builtin+trait+member candidates and relevant blanket candidates. Use an existing proven bucket if available, or agree a typed input to the existing query framework at cold source publication; no scope inventory alone proves global applicable impl completeness. Do not add a full-world map/census or unrelated-source invalidation as a shortcut. Until that proof and packet transport/cache keys are joined, this source query may establish a template but cannot close generic initializer acceptance.

Relevant template guards are exact signature/source/header/context/parse/profile/cfg/generated-source publication. Applied goal guards additionally include actual selected provider/candidate absence and source arguments. Runtime before initializer entry rechecks the applied guard and exact signature parameter coverage; after suspension it resumes the same retained application. No later source parsing or caller re-selection during return.

## Proposed bounded file/API scope (requires owner agreement)

- New frontend initializer-owner-template child/query implementation plus existing associated_constant_applications selection client; key/value/provider and mechanical exhaustive joins if made an owning memo.
- `source_types` read-only client APIs already expose WrittenTypeDeclaration::function_syntax/resolve_actual_type; coordinate any visibility/API gap with worker1 before editing. Header validation should reuse the actual Const/Static header exclusion service rather than add another spelling parser.
- Root remains owner of InitializerDemand/continuation/common signature packet gate/cache map and physical templates. This source slice does not alter those consumers or weaken the gate.

Cold cost is one exact declaration/parent header and actual Self-Type walk per source-template query generation, with shared ParseFile/AST handles. Existing tokens helper traverses excluded descendants before filtering, so header comparison can be O(actual owner subtree tokens); do not call it body-independent constant work or repeat it per activation. Hot consumers use retained template/guards and actual symbolic substitutions; no parser, body clone, full-file node search or world scan.

Required evidence: exact all-eight source review, existing signature/owner/generated-epoch controls, then actual unchanged associated-const-type-parameters first fault and full runtime application control once transport is connected. Same-source different-T/same-i32 values and unrelated caller generics not changing local-static identity are separate required controls; no full acceptance follows from this read-only proposal.
