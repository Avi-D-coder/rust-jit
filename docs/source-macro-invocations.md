# M007 selected original source invocation paths

A pinned-AST selector now produces an owned record for one exact macro-call
range. It retains ParseKey content/profile, exact range, raw path spelling,
absolute qualification and canonical enclosing inline modules. Checked path,
ancestry and text admission precedes owned metadata publication. No parser or
arena ID escapes, and no other body is enumerated or expanded. Template token
trees do not become source invocation nodes.

Original item-position selection validates source/profile/unit and enclosing
module identity. Executable calls require a later entered-frame consumer.
Prior textual producer demand remains NeedInput, including for qualified calls.
For unqualified calls, actual source textual selection takes precedence over
namespace imports. Remaining paths resolve self/super/crate, absolute extern
aliases and nested extern-prelude fallback through the retained source module
and macro namespace graph. Inaccessible local module bindings do not silently
fall back to dependencies. Original raw identifier spelling survives AST
transport; semantic lookup uses the existing canonical name implementation.

Four source acceptance checks exercise relative/raw paths, textual shadowing,
exact-range/source/module guards, body-entry rejection, dependency unit identity,
absolute qualification, template opacity and producer suspension. A macOS
Rust1.99 native fixture generates and executes four functions through original
item invocations using actual pinned adapter hygiene facts. The source comparison
checks selected original definition identity, not guest VM behavior.

The initial body range expectation included a statement semicolon outside its
expression macro-call AST node. Its failure is retained before the test range
correction. No language-profile or parser substitution was made.

Generated-token/attribute/cfg invocation integration, entered-body invocation
selection, membership seals, ordinary-item facets, local bindings and guest VM
remain pending. This API records original selected-call facts and candidate
selection; it does not establish completed expansion or release acceptance.

Original item-call eligibility now comes from the cfg-filtered outline inventory,
with exact enclosing scope/range and distinct item versus attribute producer
identities. Disabled calls and disabled modules select Absent. Active transforming
attributes and active cfg_attr expansion select their attribute NeedInput after
prior source producer demands; inactive cfg_attr remains an ordinary call.
The retained inventory contributes to outline memory estimates and has checked
count/metadata admission when building an index. Ranges remain parse attachments;
original token/blocker hashes retain semantic attribute/cfg facts.

Two source regressions exercise disabled/transforming/inactive attributes,
disabled modules and prior-producer precedence. An actual native macOS fixture
preserves an existing function against disabled duplicate generation and executes
an enabled call under inactive cfg_attr. The disabled-call before-fix witness is
retained. Active attribute execution, generated result publication, entered-body
invocations, membership completion and guest VM remain pending.
