# Coherent byte and string descriptor batch publication

This replaces `root-byte-type-batch-atomicity-v1`; do not apply both. V89 assembly guard `9c1571/1` caught that v1 had been prepared from the pre-string kernel. It stopped before modifying validation source or launching Cargo. This one-leaf delta instead starts at the exact `080f11d6...` string-chain preimage.

T040/T061 retain the complete-batch publication invariant from v1: validate the closed key sequence and all IDs/targets, reserve complete descriptor capacity/metadata, then publish keys. The string target enum and primitive Str target16 are preserved. A Ref-to-Str key is a descriptor shape only; the independent physical UTF-8 admission still controls every actual string lease. Metadata/source authority is not inferred from that shape.

The two pending registry controls include Array, Slice and Str targets across absent, partial and complete descriptor sets, plus resource and malformed-final-target refusal before logical publication. This packet satisfies the semantic v1 prerequisite of the negative-local packet without applying its obsolete patch.

Exact source hashes, patch replay and Rust1.99 formatting passed. Compilation, controls and milestone acceptance remain pending. No task, stock runtime count, canonical source promotion or JIT gate is completed.
