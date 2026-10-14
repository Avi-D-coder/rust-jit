# WT04-02 / M018 — Current original-source admission

Frozen source-only successor of sol-source-view-alias-return-v1. Uncompiled and unexecuted; no Cargo/targets/canonical source edits. M017/M018/TR003–TR005, SourceViewId tables, member/dictionary selection and actual guest value/slot admission remain incomplete.

## Actual implementation

`SourceViewPairPlan::admit_current(&Database, &PreparedSourceView, Option<&GenericEnv>, limit)` now produces an owning private `SourceViewAdmission`. The producer is attached to the existing planner as its child; source_view.rs re-exports the opaque certificate type. No caller-selected dependency list or numeric SourceViewId is accepted. The certificate owns the replayed plan, ParamEnv identity, current seal key/fingerprint and bounded full published-input membership stamps. Its stable identity domain is lrr/v1/current-source-view-admission.

Admission requires exact captured base facts, source revision/query epoch, original hygiene/declaration context, current Value discovery seal/domain/world/source graph/owning edition, ExpansionEpoch and current compiler inputs. It reparses the current exact signature including unchanged bounds/where tokens and compares the owned GenericEnv definition. Typed primitive substitutions are checked independently of the retained Param source term; nominal/foreign-Param substitutions from the legacy public capture API refuse until matched source-call admission exists. Unused absent substitution pairs remain absent.

The producer rebuilds a current entered index directly from SourceDeclarative’s pinned original-source snapshot, even for a nonmacro source. It installs only matched sealed no-item proofs against the exact body/catalog. It never calls request.expand, a query provider, guest code, rustc, native helpers or build tools. It checks current definitions and caller structure; named callee signature reads do not index the callee body or fetch BodyStructure/Thunk.

It replays the entire supported immutable provenance chain using private frontend factories: declared binding, local aliases and one-argument module named return. The latter rechecks exact selected callee/signature/argument binding and declared return substitution from current published original facts. Equal descriptors do not flatten caller Param to concrete type or replace another owner. Source term, descriptor, provenance, dependencies and retained metadata must match fresh replay. The current definition owner and retained metadata are rebuilt from exact source; cached byte counts are not typing/quota authority.

`SourceViewAdmission::revalidate` checks expected plan/descriptor and exact full input membership, then repeats current source replay and verifies ParamEnv/seal/world identity. Any publication/addition/removal/change conservatively invalidates; old query-revision plans need a new source-plan mint. No metadata is attached to VM storage by this patch.

Metadata uses checked sizes, 100k/32 MiB root stamp limits, provenance256/dependencies32 existing planner limits, and producer retained bytes capped at32MiB/caller limit. Stamp row accounting includes sizeof<(InputKey,Digest)> plus conservative128 tree overhead and owned key heap. Runtime descriptor storage remains separate; source/member identity is never recovered from a descriptor.

## Prelude and other typed prerequisites

No established exact Value-prelude/pattern witness exists in the current interface. Ambiguous BindingOrConstant rows remain deferred via an empty resolution map; only pinned explicit binding syntax can mint. No module-absence fallback, guessed identifier whitelist or blanket Binding decision remains. The exact authority decision is retained in authority-frontier.md. Unknown cold binding names stay deferred; only reached lookups needing one refuse.

Generated/native/foreign/dependency worlds, lexical owners/callees, incomplete multiargument return provenance, const/region substitution, composite terms, general source-call substitutions and resolved member/dictionary ParamEnvIds retain typed frontiers. Current owned bound token recipes are retained exactly without resolving/forcing them. This certificate is currentness authority for the bounded original plan, not a complete source member/call contract or a VM admission success stub.

## Patch/API dependencies

1. Apply the preserved alias/return prerequisite complete patch (including its syntax planner/tests).
2. Apply planner-prerequisite.patch: tiny admission child attachment plus pure published_input replacements for old &Database readers. Frozen prior artifacts are unchanged. Mutable named-call factory query observations remain its existing path; the new producer does not call that query path.
3. Apply production.patch and acceptance.patch, or complete.patch containing the four exact changed/new files.
4. Root owns memo attachment/implementation. The exact formatted root draft at artifacts/macos-first/root-source-view-inputs-v1/candidate/crates/lrr/src/db/source_view_inputs.rs supplies:

   - source_view_namespace_stamps(&self) -> Result<BTreeMap<InputKey,Digest>,Diagnostic>, including every published key and fingerprint with membership/absence represented by exact equality; bounded before clones, pure/no trace/provider/input mutation.
   - source_view_scope_certificate(&self, &SourceSelection, &ScopeKey) -> Result<Arc<InputValue>,Diagnostic>, exactly one current matching Value seal with source/edition/domain, ExpansionEpoch, SourceDependencies and compiler-input validation. Missing/ambiguous/incomplete authority returns typed refusal.

The interfaces match read-only inspection. Root was notified that its initial fixed128 row charge should additionally include sizeof<(InputKey,Digest)> to cover key/value storage; producer already uses the stronger checked charge. No root memo file was copied or edited.

Root must still wire certificate revalidation and checked actual GuestStore value/descriptor/slot/owning GenericEnv checks before any runtime SourceViewId attachment. The earlier legacy frame transfer witness alone does not grant general plan admission. VM/IR/engine integration and portable mappings remain root-owned dependencies.

## Acceptance scope

Eleven source-only controls are preregistered/drafted outside originals. Unchanged copy-trait-implicit-copy and generic-fn-infer capture original all4-edition source sites and preserve the typed Value-prelude/pattern prerequisite; they do not mint around that blocker or claim full fixtures. Copy main is empty. LRR explicit nonmacro parameter/alias controls exercise mint/revalidation all4; explicit alias→named id return retains caller Param despite I32 descriptor. Negative controls cover unsealed plans, changed epoch with same source revision, derived seal membership publication, wrong source/ParamEnv/bounds, zero budget, changed term/provenance/dependency/metadata fields, forged certificate membership, and unproved nominal substitutions through legacy public contracts. No external field can construct this private certificate; test tampering stays inside its private child.

Upstream originals/licences are byte-identical to ae0fe511b15ab760d7e963b13389d1bc2ab42b18 and hashes retained in upstream-provenance.json. Generic-fn-infer already appears in MAIN manifest; Copy requires the unchanged importer before compiling. No auxiliary/revision/expected-output companions exist for these run-pass cases. No full-main/guest execution or provider/native acceptance is claimed.

## Checks and next validation

Pinned Rust1.99 rustfmt with skip_children=true and exact-base git apply --check are source checks only. Production source remains frozen for root’s compilation/targeted async validation. Root should validate new producer controls, preserved propagation controls, source input/epoch/seal regressions and required integration milestone after resolving each explicit prerequisite. No worker test/build pass is claimed.
