# Draft GitHub invitation

We are building LRR, a Rust interpreter that prepares semantics when execution reaches them and will later optimize observed code through the pinned Cranelift/LLVM architecture. We are looking for contributors who can bring implementation time, model budget or agents and submit reviewable PRs.

There are 12 substantial work streams under worktracks/: upstream contract execution, numeric semantics, control flow/patterns, calls/source views, traits, ownership/cleanup, library runtime, macros/editions, Cargo preparation, query caches, authority/workers and integration/release. Each has bounded starting slices, task mappings, ownership boundaries and acceptance requirements. Choose a slice rather than allocating agents to an entire ambiguous feature area.

Our immediate priority is complete interpreter acceptance of applicable official rustc runtime contracts on macOS ARM64/Rust 1.99.0, across Rust 2015/2018/2021/2024. Unsupported runtime features remain blockers. Compiler diagnostics/tooling have separate reports; runtime-only success in a mixed fixture is not a full-fixture pass. Guest JIT starts only after runtime and interpreter correctness acceptance. Linux is a later port.

Use the Worktrack claim issue template, coordinate exact files and interfaces, work in your own fork/branch, and submit the PR template with unchanged upstream fixtures and reproducible asynchronous validation evidence. We maintain plans after merges and during active weekly triage. Human owners review their agents' work; imported tests and project component checks are not stock fixture passes.

Start at worktracks/README.md and AGENTS.md in this repository. Current baseline: 243 imported upstream cases / 398 unchanged files; full upstream fixture passes are 0. See current implementation-state and inventory reports for actual coverage. This paragraph is a snapshot and must be refreshed before posting.

Publication checklist: configure/verify the intended repository remote, push reviewed plans, replace the paths above with repository links, refresh counts/pins/active claims, and then publish through the owner's authorized GitHub workflow. This file is a local draft; no announcement or issue has been posted.
