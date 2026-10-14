# M013 helper inheritance through attribute expansions

The supplied rustc resolver (`compiler/rustc_resolve/src/ident.rs`, helper-scope
walk) follows helper bindings through attribute expansions and stops at bang and
derive expansions. Its unchanged `derive-helper-shadowing.rs` case is copied with
both edition directives, auxiliaries and revision snapshots under MIT OR
Apache-2.0. The component assertion verifies its field helper, pending legacy
outer helper and sibling isolation; it does not pass the full revisions.

The original native identity-attribute witness completed both derives and the
following attribute but then blocked on the retained helper.
`m013-helper-inheritance-before-production.log` records this implementation gap.
The later observation run reached sealing but used a named-field struct in a
value-namespace lookup; that was a harness mistake, not an implementation witness.
The final tuple-struct control legitimately exercises both the constructor lookup
and outer/field helper inheritance.

Exact helper proofs now retain hygienic provider names and introduction contexts
with explicit stable encoding. An attribute child inherits only its parent's
already introduced occurrences and inherited attribute environments. The existing
exact pending-request, output, parent, cfg and epoch checks bind that transition.
Bang and derive children receive no inherited environment. Bare helper matching
uses pinned item-normalized contexts; qualified/imported spelling is not granted
helper status. No helper is registered globally by name.

Known inherited attributes remain in forwarded tokens while being excluded from
replacing-attribute execution. Original output items whose enabled attributes and
descendants are settled receive a checked proof fingerprint, allowing their
ordinary routes to publish. Classifications replay before sealing. Retained
environment and item metadata contribute to the existing checked discovery
budget; count and allocation limits remain enforced.

The identity control records five native calls and two derive outputs. It retains
the tuple constructor, derived constant, outer helper and field helper, then seals
and publishes through the owning run. The bang control records the same five
native calls plus one declarative expansion, remains catalog-incomplete at the
generated helper, and refuses publication. Its incomplete outcome is an explicit
barrier regression, not a passed language fixture.

Full copied-case execution, complete nested/module/source helper inheritance,
legacy compatibility, original-source and builtin integration, larger resource
matrices, remaining M013 contracts and later milestones remain pending. M013,
release and Linux acceptance are not complete.
