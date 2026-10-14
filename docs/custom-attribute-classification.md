# M010 custom attribute classification

`frontend::item_attributes::classify_custom_attribute` handles item-local helpers
and ordinary procedural attributes after compiler-owned builtin identity handling.
It reads the exact owner/cfg/active derive scope and frozen procedural export
catalog; it executes no macro and grants no authority.

For an unqualified attribute, missing active helper declarations produce
`NeedHelpers`. A ready positive helper scope precedes ordinary namespace candidates,
including imported attribute/function macros and an unresolved namespace input.
Qualified paths bypass item-local helpers. Otherwise a pending namespace input
produces `NeedNamespace`, a known attribute export returns its exact procedural
identity, and an opaque definition outside the prepared catalog produces
`NeedDefinition`. Absent, inaccessible, ambiguous or wrong-kind namespaces are
diagnostics. No unknown attribute becomes an inert slot by spelling. Helper lookup
also rejects active keys that describe a non-derive invocation.

The precedence is measured with pinned native Rust 1.99.0 on macOS. An active
`MemberCfg` helper retains the original item and emitted member constant despite
an imported attribute or function macro of the same name. A qualified attribute
and an imported attribute without an active helper replace the item. The acceptance
builds and runs native test harnesses, verifies retained-item tests versus generated
test discovery, and checks native rejection of an absent attribute. LRR decisions
use an exact source-facet helper declaration admitted against controlled schema4
preparation and loaded exports. Classification and helper/catalog lookup leave the
LRR macro-session invocation count at zero. Missing declarations, namespace and
definition inputs remain typed suspensions; foreign owner and non-derive active
keys refuse.

Compiler-owned derive/test identity classification now integrates through
[builtin attribute facets](builtin-attribute-identities.md); full builtin handling
and automatic compiler/sysroot facets remain pending. Adapter-backed
[builtin derive helper metadata](builtin-derive-helpers.md), including `Default`/`default`,
now integrates with the same scopes and custom attribute classifier. Automatic item
query/worklist orchestration, generated-source rebuilding, discovery and guest
execution remain pending. This component does not complete MX018–MX020, M010, release acceptance
or the later Linux port.
