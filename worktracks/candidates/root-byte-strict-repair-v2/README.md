# Strict byte cleanup follow-on

Source-only mechanical T041 correction after V75 strict terminal `51d1d6/101`, which resolved the missing imports but reported two `collapsible_if` errors. Collapse the failure/membership/abort chain with the same short-circuit order, preserving its abort-on-invalid-membership behavior and terminal error propagation. Apply pinned formatting to the newly added VM trait import. No lint suppressions, production semantic changes or upstream edits.

Verify exact before/after hashes before resuming interrupted strict/scoped validation. Later assertion assembly must retain both this formatting and the prior production trait imports. No runtime pass, task completion or integration is claimed here.
