# E-M007-005: preserve the producer's source interval

The initial local-scope publisher checked that generated positions followed the
producer and did not duplicate existing positions. It did not bound them by the
next known statement or child scope. A producer at 10 could publish bindings at
15 and 25 despite a following source frontier at 20. This changes local visibility
rather than faithfully inserting the producer's output at its original position.
The minimal before-fix regression fails in
`artifacts/macos-first/m007-local-hygiene-frontier-before-correction.log`.

Record child entry points as source frontiers and require the whole published
batch to fit strictly between the producer and the next known statement/producer
or child-entry point. Use excluded BTree range bounds rather than unchecked
position arithmetic. Invalid output refuses before names, definition metadata,
or pending demand change. Existing source positions and child capture points
remain intact. The source scheduler must assign sufficient logical source-order
space or construct a new consistent snapshot; positions are not byte offsets.

The regression covers a following binding and a following child scope. It first
attempts a valid-then-out-of-interval batch, verifies demand remains, and then
publishes a valid batch. This repairs the new LRR index implementation, without
changing Rust rules or the pinned frontend's normalization/resolver algorithms.
Full source discovery integration remains required in M010.
