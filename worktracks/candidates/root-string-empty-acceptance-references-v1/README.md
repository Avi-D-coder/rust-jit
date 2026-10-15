# Register the pending string emptiness controls

T061 requires explicit source-derived semantics and refusal evidence. This three-leaf follow-on adds the two str::is_empty candidate function names to T061 acceptance_tests and registers matching fixture IDs in compat/fixtures.toml. Both entries stay pending; the upstream originals remain unexecuted full fixtures. Existing task status and completion fields are preserved.

Apply only together with root-string-empty-v1 after the frozen schema17 milestone is collected and promoted. The exact V97 preimages, parsed JSON/TOML, unique IDs and no-fuzz/no-offset replay were checked. The actual fixture-reference audit and Rust execution remain pending. These metadata references provide no runtime, full-fixture or task acceptance. Preserve newer canonical candidate/evidence fields when merging docs/tasks.json rather than overwriting the whole document.

The existing match_no_arm_exit control contains a const assertion that LRIR_RECIPE_SCHEMA is17. The source-selected is_empty operation deliberately advances this to18; this packet updates that exact assertion to18, preserving the control. This was caught by code review before compiling the held successor. Rust1.99 formatting and exact replay passed; no compiler or runtime result is claimed.
