# E-M007-010: validate every inherited declaration row

The initial multi-source extension selected the first inherited metadata row
for a definition identity. A later captured source could supply conflicting
name context/origin metadata for that same identity without being checked.
This violated the existing requirement to reject conflicting member identity
before choosing or filtering a candidate.

The minimal owned-facts regression uses three declaration tables with one
identity: local and first inherited contexts agree, while the second inherited
context differs. Its actual failing assertion is retained in
`artifacts/macos-first/m007-linked-module-before-metadata-fix.log`.
`inherited_source_chain_checks_every_definition_metadata_row` now checks every
matching table row before name filtering or origin selection. Repeated identical
rows remain valid. The original pinned-name/origin algorithm remains unchanged;
this is input consistency validation, not an alternate resolver.
