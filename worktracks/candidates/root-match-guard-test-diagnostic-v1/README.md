# Format concrete guard-test outcomes

V118 strict compilation exposed E0277 in the new test's failure message: RunPoll does not implement Debug. This one-leaf follow-on matches the concrete Completed/Fault/NeedInput/Panicked variants, preserving the same successful-completion requirement and reporting the actual typed error on failure. No production code or acceptance oracle changes.

Apply after the guard/result packet and before the final shared-ledger alignment. Formatting and exact replay are checked; strict compilation, three new acceptance bodies, affected controls and a fresh full milestone remain pending. The V118 compile failure is retained, with zero test bodies or upstream passes inferred.
