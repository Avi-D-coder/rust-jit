# Byte assertion strict test lint repair

The joined byte assertion candidate type-checked production code but V85 strict validation stopped on two test-only Clippy lints (`655b41/101`), before any test body ran. Collapse the nested literal-kind guard and use fixed-size slice chunks for the two physical operand pairs. Inputs, branch conditions and assertions remain identical. This source-only two-leaf repair needs strict validation before the queued original CRLF main and affected controls; no pass is inferred from lint repair.
