# M010 cfg preprocessing within derive inputs

The native Rust 1.99.0 macOS probe distinguishes outer item and member cfg:
outer cfg/derive attributes are removed, whereas enabled fields and variants
retain a positive `cfg` attribute. Active cfg_attr expands into its effective
attributes. Disabled fields and variants disappear. A disabled variant suppresses
invalid predicates in its nested fields. The initial failed assumption that all
member cfg attributes disappear is retained in
`artifacts/macos-first/m010-member-cfg-native-initial-witness.log`; this measures
native conventions and does not contradict the specification.

`ParsedFile::derive_item_inputs_at` uses the pinned AST to capture owned field and
variant ranges and effective attribute facts after outer cfg admission. The
traversal stays within the owning ADT and skips disabled variant descendants.
Disabled range lookup uses an ordered index. Member count and metadata byte sizes
are checked. `expand::member_cfg::derive_input` consumes those facts and the exact
original forest, rejecting mismatched source/cfg/owner facts and duplicate ranges.
It filters disabled member token ranges and following separators, reconstructs
active member attribute subtrees, and retains delimiters and original span/context
records. It performs no macro execution, type checking, or file reads.

Six actual native comparisons cover record fields, tuple fields, enum variants
with nested fields, and all-disabled record/tuple/enum lists. A matched external
macro returns the identifiers it actually receives, which agree with native Rust
for each sample. A second actual identity expansion passes each retained item
through the pinned item parser, checking separators even for empty lists. Twelve
explicit native tool invocations are observed. Unit checks exercise nested
cfg_attr order, positive cfg retention, disabled descendant suppression, original
record retention and stale/duplicate member refusal.

The existing outer-only derive input API retains its stated component scope.
The new member-aware API must be used when source planning integrates full derive
requests. Automatic generated-token planning, source-aware worklist reanalysis,
helper metadata, whole-file publication, guest execution and full MX018–MX020
remain pending. No full task, release or Linux completion is claimed.
