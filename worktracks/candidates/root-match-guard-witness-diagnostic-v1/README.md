# Retain the actual upstream match-prefix failure witness

V119 passed the new guard exact-once/cold-path and independent numeric-island controls. The unchanged overlapping-match function component failed with TYPE-CONTEXT before its expected static frontier. Its existing assertion did not print the actual message, original source position or completed prefix. This test-only diagnostic addition reports those facts before the unchanged error-code/message/source/counter oracles. No runtime behavior or successful outcome is introduced.

Apply after the previous RunPoll diagnostic repair. Recompile and run only this failing original component to collect the actual first frontier; the other two controls need no repeat on this diagnostic-only edit. This is a diagnostic probe, not an expected passing scope, and all existing failures remain retained. Formatting and exact replay are checked; execution and a corrective production implementation remain pending.
