# Byte scope failure diagnostics and real type namespace preparation

V78 strict builds passed and its scoped library run reported 64 passed/4 failed. The range-binding runtime harness prepared only value namespaces although its source-aware lookup requests the type namespace. Prepare both through actual SourceDiscovery and publication; no fabricated seal is installed.

The two failing byte-declaration controls gain exact PC/operation/edition diagnostics while preserving their assertions. Their cause is not yet established; this packet is diagnostic reproduction, not a fix or passing outcome. The separate unresolved initializer macro failure remains under investigation. No upstream original or production behavior changes.
