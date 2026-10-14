# M010 effective attribute candidate membership

Original outlines retain each effective attribute's exact meta range as a source
attachment. The attachment is excluded from semantic header encoding: moving an
unchanged declaration must not change its semantic header merely because a byte
offset changed. Serde can read an older missing attachment, but original candidate
index construction refuses it instead of inventing a range.

`SourceMacroIndex` records bounded effective-meta ranges by original item scope
and owning declaration range. Candidate lookup now requires this membership in
addition to matching parse/crate/module context. Disabled items and inactive
cfg_attr children do not become eligible even when their paths resolve in the
namespace. Missing candidate membership returns Absent without certifying namespace
completion; missing, duplicate, excessive or escaping source attachments are
diagnostics. Field/variant attributes carry member/container entry requirements
and cannot use the original item-header route.

The acceptance compares a disabled item, inactive nested cfg_attr meta, enabled
attribute and field attribute in the same captured source. Only the enabled item
meta routes to the disclosed definition. Missing/escaping attachments refuse;
changing attachment data leaves semantic header identity unchanged. The native
builtin/procedural alias fixture continues to use effective indexed attributes.
No helper executes, body is entered or source file is read during candidate routing.

Entered member/body attribute binding, namespace seals, automatic item query
orchestration, compiler/sysroot facets, generated-source rebuilding, discovery and
guest execution remain pending. This is component evidence and does not complete
M010, full MX018–MX020, release acceptance or Linux support.
