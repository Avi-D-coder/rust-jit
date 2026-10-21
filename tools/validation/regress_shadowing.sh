#!/bin/bash
# LRR-specific control (not an upstream rustc fixture): bindings that shadow a function parameter
# (`let p = p + 1`, `match p { p => .. }`, closure/for/if-let patterns, macro_rules hygiene, a shadow of a
# different type so a wrong parameter read is visible). Each program in tools/validation/shadow_params asserts
# its values and must exit 0 with `"status":"completed"` through the production CLI. A fast regression screen only.
# Usage: tools/validation/regress_shadowing.sh [binary] [editions...]   (default editions: 2021)
ROOT=$(git rev-parse --show-toplevel)
BIN=${1:-$ROOT/target/debug/lrr}
shift
EDITIONS=${@:-2021}
fail=0
for ed in $EDITIONS; do
  for f in "$ROOT"/tools/validation/shadow_params/*.rs; do
    out=$(LRR_EMBEDDED_FACADE=1 timeout 300 "$BIN" --edition "$ed" run "$f" 2>&1); rc=$?
    if [ $rc -eq 0 ] && echo "$out" | grep -q '"status":"completed"'; then echo "PASS $(basename "$f") ($ed)"; else echo "FAIL $(basename "$f") ($ed) rc=$rc $(echo "$out" | head -1 | cut -c1-140)"; fail=1; fi
  done
done
exit $fail
