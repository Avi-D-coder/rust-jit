# Bounded pure-loop/local-state source successor

This source-only patch is based on the separately published interpreter-constants candidate v63, not MAIN production alone. Apply its code only after the predecessor candidate and exact before hashes match. All16 controls are uncompiled, and all4 copied upstream cases remain pending. Review.md states required work and evidence limits.

For integration with the local official checkout, use tools/import_rustc_tests.py to merge these four cases into the canonical manifest. Do not replace the complete canonical manifest with this small packet manifest. Keep originals unchanged under their retained MIT OR Apache-2.0 license. No validation logs, target directory or workspace snapshot is published here.
