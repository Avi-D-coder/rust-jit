# Byte descriptor batch publication

T040/T061 storage invariant: a refused byte-literal/string backing descriptor batch must not publish a descriptor prefix. This follow-on changes the shared byte descriptor kernel; it does not grant source type, trait, lifetime or UTF-8 authority.

The literal path previously interned its predicted array/slice/reference keys one at a time. The annotation path already reserved its complete two-key batch. Both now use one closed, at-most-three-key commit: validate current count, exact consecutive IDs, unique missing keys and existing/earlier byte targets; check complete metadata; reserve the full descriptor vector; account for actual allocator capacity; then intern the validated keys. A malformed later key or a resource refusal therefore occurs before the first logical insertion. Capacity retained by a failed reservation remains part of worker metadata accounting.

Two pending LRR controls cover resource fault injection after a pure preview, a malformed final target, and reuse with absent, partially present and complete descriptor sets. These exercise LRR-specific publication/resource guarantees, not upstream language compatibility. Existing annotation/count/staleness tests remain in place. The pinned upstream string and CRLF programs in the string candidate lineage still supply the language witnesses; this packet imports no new original and claims no runtime pass.

Apply after the frozen V88 source and the string lineage (through `root-string-byte-view-v1`); this leaf is unchanged by that lineage. Exact patch replay and Rust 1.99 formatting are checked. Compilation, all four registry controls, affected byte/string banks and a fresh full milestone remain pending. No task or canonical promotion is complete.
