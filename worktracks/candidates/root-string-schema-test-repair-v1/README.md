# String schema assertion repair

The coherent string candidate advances LRIR to schema 14. Its existing `xtask/tests/match_no_arm_exit.rs` compile-time assertion still pins schema 13. This mechanical follow-on updates that assertion to 14; the runtime exhaustion test and its expected behavior are unchanged.

Apply after `root-string-call-coherent-v3`, before compiling it. The separate `root-string-length-v1` candidate advances this same assertion from 14 to 15 along with its new opcode. Exact replay and formatting passed; compilation and execution remain pending. No runtime or full-fixture pass is added.
