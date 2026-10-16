# Semantic header identity across source normalization — LOCAL-024

M014/T024, revision 5 Sections 6.2, 50 and 57; October 16, 2026.

Actual unchanged witness: `borrowck/borrowck-assign-to-subfield.rs` main on
source `e4e4a81e` fails before guest observation with
`original declaration normalization header/body differs`. The same source's
12 selected component controls pass; both whole upstream mains still fail.

`SourceAssembly::new` starts from the exact captured caller tree, before any
expansion. `assembly_anchors::capture` pairs original declarations with their
normalized structural counterparts by actual definition identity. It currently
compares `HeaderRecipe` using derived equality. That equality includes
`AttributeRecipe.source_meta`, which is explicitly a source attachment excluded
from semantic header encoding. Normalization can move the attribute while its
tokens, class, order and owning profile remain identical.

Extend `HeaderRecipe` with an explicit semantic comparison of every encoded
field, excluding only attribute source attachments. Use it at this exact
original-to-normalized anchor join. Keep derived equality and all other source
currentness checks unchanged. Do not discard attributes, accept arbitrary
header differences or change original declarations' authority. Actual normalized
declaration/header/body ranges remain in the anchor, and original ranges remain
separate. Generated syntax still enters ordinary indexing and execution.

The regression uses the complete unchanged official witness in all4 editions,
checks preserved semantic headers and distinct actual source attachments, and
requires ordinary source assembly to succeed. Attribute token mutation must
still fail the semantic comparison. This is source-anchor evidence, not guest
execution, cleanup, a full fixture pass or all8 architecture acceptance.

Modes 1–5 add no evaluator, macro-name rule, type/pattern or method logic.
Modes 6–8 add one bounded cold header comparison at an existing preparation
join, no dispatch planning, namespace census or BodyIndex copy. The installed
constructor planning violation remains held separately. Freeze exact sources
and collect the upstream-main result before claiming runtime progress.
