# Repair the remaining H002 fixture selector

The V95 condition control passed with its positive string assertion. V96 updated the docs/tasks.json selector, but its actual fixture-reference audit failed because compat/fixtures.toml still named the obsolete negative control. This one-leaf follow-on updates only H002-CONDITION-BINDING-7 to the existing, tested function. It changes no Rust source, fixture bytes, assertions or applicability.

Exact released V96 preimage and no-fuzz/no-offset replay passed. Run the real fixture-reference audit before the fresh combined milestone. V95 scoped results remain evidence for their frozen Rust bytes; this metadata change does not count as a Rust rerun or full-fixture pass. Canonical promotion and task acceptance remain pending.
