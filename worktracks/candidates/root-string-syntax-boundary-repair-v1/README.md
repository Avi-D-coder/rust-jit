# Preserve the syntax boundary and the supported string condition

T022/T024/T041/T043 invariant: parser and AST identities remain in the syntax layer, including fixture adapters; storage and VM tests receive owned strings, structural indexes and verified recipes. Existing accepted string literals cannot retain a negative “unsupported” expectation.

V94 full milestone failed with3328 listed bodies:1973passed,1failed,1354unexecuted; all1192 library bodies passed. Its architecture audit separately rejected parser imports in four runtime/VM test leaves. The sole host failure expected `fn case()->i64{if let owned="owned"{3_i64}else{missing()}}` to return the old literal-kind unsupported fault. The failure only established a non-Fault outcome, not a verified return value.

This seven-leaf repair moves the three exact fixture extraction routines into test-only syntax/string_fixture_facts.rs. Runtime tests still check every upstream token, UTF-8 byte, descriptor/root/lease, cached view, cursor, refusal and original VM recipe. The same counts and unchanged originals are retained. No parser import is hidden behind qualification or an audit exception. The condition control now requires actual Signed(3), exactly one binding operation, balanced operations, and no demand/fault/panic from the unentered else. It does not count that LRR-specific program as an upstream fixture pass.

No production runtime behavior or LRIR schema changes. Exact V94 preimages, Rust1.99 formatting and no-fuzz/no-offset replay passed. Strict lint, architecture, affected controls and a fresh milestone remain pending. The separate schema18 is_empty successor is held and must not be overlaid during this repair's validation. No canonical promotion, task/JIT/full-suite acceptance is claimed.
