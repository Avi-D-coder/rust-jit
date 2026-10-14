# Exact prepared-source selection for macro lookup

`PreparedSourceInputs::select` takes a SourceSelection containing the immutable
path and complete OutlineKey: content, live profile, cfg, unit and canonical
logical module context. It returns Selected, Absent or Changed with the current
key. Source selection validates profile/module-path size before owned module-context
publication. Equal source bytes with different cfg are different selections.

A selected PreparedSourceView retains its immutable prepared publication and
borrows that publication's outline. PublishedMacroIndex builds only from that
view and requires the exact canonical selection on each lookup. A changed
content/cfg/unit/path request cannot silently use the retained index. Holding
an old view intentionally permits lookup of that old source revision after a
new publication or deletion; its source identity remains explicit.

The view retains the complete prepared revision. Existing weak source registry
accounting charges its source bytes, parse and outline metadata while retained.
The acceptance test removes all other owners, verifies both retained revisions
remain charged, then verifies the source-byte charge drops as each index is
released. This does not claim complete expansion-arena/deadline accounting,
which remains M012 work.

The native original-source fixture now selects an exact published view before
outlining lookup and expansion of selected original source slices. It preserves
the previous macOS comparison [17,29]. Pure acceptance covers edits, deletion,
cfg-only changes, unit/path mismatch, canonical module contexts, profile rejection
and path depth limits. Source files, macros and build tools are not executed by
selection or lookup. Entered-body discovery, selected external modules, import
paths, generated epochs and later guest execution remain required integrations.
