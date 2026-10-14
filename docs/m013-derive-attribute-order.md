# M013 derive groups and following attributes

The unchanged rustc `derive-expand-order.stdout` is the payload oracle for five
derive occurrences. Its first derive receives later derive annotations; Third
and Fourth share an annotation and both receive only the following Fifth
annotation. The former materializer removed every derive annotation at once.
`m013-derive-attribute-order-payload-before.log` records that implementation
failure. The new component assertion compares all five ordered identifier
sequences against the original snapshot and checks retained source records.

The unchanged `attribute-after-derive` case specifies transformation order.
Its snapshot shows configured derive input with disabled fields removed, while
the later attribute receives the member `cfg` tokens. These are different
payload projections. A queued later attribute must neither block the earlier
derive nor become globally inert. Unknown preceding attributes and unresolved
member helpers still block. The pre-fix native control stopped after the two
initial attributes, before either derive executed; its before log records this
second implementation failure. Neither failure contradicts the specification.

Actual derive requests now materialize through their own annotation group,
retaining later groups and attributes. The discovery queue executes and appends
the current group before advancing. Only certified completed groups are consumed
when preparing a later attribute. Exact helper decisions suppress helper
execution while preserving their tokens in the attribute input. Original item
tokens, spans and identity remain immutable.

The later replacing attribute uses the same bounded, immutable parent prefix
view as derive children. Its child sees earlier outputs, while final selection
replay uses that same view. Prefix publication preserves earlier bindings before
retiring the original producer. The original item route remains unpublished when
the later attribute replaces it.

Two native controls cover one annotation containing two derives and two separate
annotations. Both require five native calls: two initial attributes, two derives,
then the following attribute. Recorded execution/append events distinguish the
two grouping orders. The SDK asserts disabled fields are absent from derive
input, member `cfg` and helper tokens remain in attribute input, and consumed
derive annotations are absent from attribute input. Discovery retains the derived
constant, excludes the old item, finds the replacement, seals and publishes
through the owning run. The pure request control also keeps a later unknown
attribute explicitly unresolved and refuses direct procedural execution.

The rustc files remain unchanged under MIT OR Apache-2.0. LRR adaptations and
authority checks live outside the licensed directory. These are component
assertions and controlled native executions, not whole copied-fixture passes.
Empty derive annotations, broader transformation/namespace/nested/import
matrices, legacy helpers, original-source and builtin payload integration,
remaining M013 contracts, later milestones, full release and Linux acceptance
remain pending. M013 remains in progress.
