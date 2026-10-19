# Shared original item targets — M019/M020 prerequisite

Solo root, LOCAL-049 coordinated with root-owned LOCAL-037/044. Baseline
04bf4384c7471670a5ea6d64b65c578f374d2bdd. Unchanged primary witness:
tests/ui/coherence/coherence-impl-in-fn.rs, all four official editions. Its
89..108 derive Meta belongs to a real body-local enum; module Outline lookup
cannot identify it. The global catalog must remain held until real expansion,
publication and header closure work for this item. This is not permission to
skip the derive or synthesize its implementations.

The common mechanism is ParsedFile::declaration_scope_plan -> the existing
lexical_declarations/BodyItemBudget::collect -> DeclarationFacts. Ordinary
entered_macro_facts and LocalImplOwners already use it. Extend retained source
item selection through these exact plans, preserving their canonical DefKeys,
scope owners, cfg, editions and true syntax ranges. Module items keep their
actual Outline rows. No executable BodyIndex, binding preparation, source call,
Thunk or pattern plan may be demanded to identify an annotation target.

First retain and share the canonical plan, rather than walking a body again for
each reader. Deferred binding requests must retain direct syntax locations in
the same immutable parser tree, so the plan can be shared safely without
thread-local syntax nodes, full-file searches or copying a BodyIndex. Binding
preparation still occurs only at entry. Remove the existing whole-source
ParsedFile::parse calls in original derive and attribute selection in favor of
the publication's retained parser. This removes a production alternate parser
route; it does not by itself deliver cold derive expansion.

Then expose an actual original-item target which retains its true Outline or
canonical local plan/row, attached to the selected immutable publication.
Selecting a local target follows genuine enclosing item bodies and does not
invent owner identities from the declaration scan's coarse function owner.
Unknown/generated parents stay explicit gaps; generated syntax continues
through its admitted expansion route. The shared target must be usable by
attributes and derives; no Copy/Clone/derive spelling changes runtime behavior.

Preparation keys include actual parser, cfg, body handle, scope and header
context. One cold lexical traversal per key; repeated readers reuse immutable
plans. Direct syntax-location resolution is proportional to tree depth, not a
whole-file search. No new per-operation/dispatch work, namespace census,
resource budget, sandbox check, static checker or native loading is introduced.
Source publication/epoch changes select different parser/plan owners.

Closure requires unchanged upstream local-item witnesses, real canonical
identity and retained-handle equality against ordinary entry across editions,
module/body attachment and changed-source rejection, no executable query
demand, and eventually the original failing catalog matrix plus generated,
wrapper and standard-macro contexts. Component evidence cannot close M019,
M020, macro architecture or runtime/JIT gates. Ordinary completion/append and
DeriveDeclarationOrigin module-only assumptions remain named repair work until
replaced and validated. Review all eight recurring architecture modes against
the exact validated source fingerprint before any checkpoint/integration claim.


Implemented bounded prerequisite: retained canonical lexical plans (including
pinned direct deferred-binding locations), real original module/local item
targets, a once-built exact range index, and removal of four alternate full-file
parses. Ordinary module derive/attribute consumers use the shared target; the
unchanged local enum attaches to the same canonical plan as ordinary entry.
The existing parent body/content identity validation before a cache hit remains
an O(body syntax) cost. No constant-time entry or complete performance gate is
claimed. Local derive grouping/origin/append/completion integration remains held.
V255/V256 terminals and exact-source review accompany the checkpoint.
