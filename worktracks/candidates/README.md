# Source-only implementation candidates

These compact patches are pending contributions within the existing twelve worktracks. They are not additional workstreams, merged implementations, task completions or upstream fixture passes. Frozen worker notes retain their original local artifacts paths and exact patch hashes; full local snapshots, targets and raw validation outputs are ignored. Read the named task, dependency and source-baseline requirements before applying any patch to an isolated checkout. Coordinate shared hooks through the owning claim.

`interpreter-constants` preserves the current root source candidate as a delta against this checkpoint. Other folders contain source-only worker handoffs and their predecessor patches; their Rust controls remain uncompiled unless the local implementation-state explicitly records an actual later terminal. `root-constant-loop-control-v1` contains a new uncompiled root successor with unchanged licensed upstream inputs.

Source checkpoints are pushed to the implementation branch. Pull-request code is fetched or applied only after explicit user approval. Build artifacts and complete copied workspaces stay local. Run tools/check_source_checkpoint.py on staged files before publication.
