# M007 linked original local-name lookup

LinkedEnteredMacroIndex now builds LinkedSourceLocalIndex by validating the
complete external-module source chain and delegating to the existing immutable
original local index. The wrapper borrows the same LinkedModuleIndex. Every
lookup checks current/parent/root source selections before indexing the current
source or consulting locals. Existing exact body/range/opaque guards, typed
pattern outcomes, pinned hygiene and declaration-order rules are preserved.
Nested-function and container-function entry return the same linked body type,
so local lookup through either path uses these guards and isolated local roots.

A three-file source acceptance covers block shadowing, parameters, nested-function
isolation, local-container function entry, stale current/child/root sources,
missing ancestors, changed body keys and lookup inside opaque nested syntax.
An actual macOS Rust1.99 program executes four corresponding values. Each chosen
binding's literal initializer is read from the retained fixture source; selected
original values match native execution. This proves the selected original local
binding identities with actual pinned root hygiene. It does not evaluate arbitrary
constant expressions or execute guest LRIR. Marked collision behavior retains its
separate original-source/pinned-mark native acceptance.

This closes linked-wrapper integration for the original local index. Additional
binding forms, closure capture, pattern equivalence, generated-local publication
and query/service consumption remain pending M007/integration work. M012 global
accounting and later VM/release work remain required. Evidence:
artifacts/macos-first/m007-linked-locals-checkpoint.json. Release remains incomplete.
