# E-M007-006: retain emitted binding history until hygiene selection

This corrects an implementation contradiction with revision 4's hygienic
source-member selection; it does not change the specified frontend or semantics.
The older lexical-only `TextualScopes::publish` overwrote every repeated
canonical spelling at one producer position. Once pinned item-name comparison
is joined to lookup, a later incompatible name must not erase an earlier
compatible name from the same output.

The minimal owned-facts witness publishes two definitions of `m` at one producer
position with distinct item comparison classes. A reference matching the first
class incorrectly returned absence. The actual failing test output is retained
in `artifacts/macos-first/m007-textual-selection-before-history-fix.log`.
The regression is `same_output_incompatible_latest_name_does_not_erase_compatible_definition`
in `crates/lrr/tests/hygienic_macro_scopes.rs`.

Each producer/name position now retains a bounded vector in emission order.
Candidate traversal reverses positions and vectors, then the joined lookup
filters using owned pinned comparison classes before origin ambiguity selection.
The provisional lexical API still chooses the last spelling. Existing same-name
shadowing and the seven actual macOS native origin cases remain acceptance
requirements. The witness supplies distinct comparison facts; it does not claim
source discovery, procedural execution or guest VM acceptance.
