#!/bin/bash
# Retained-pass quick check: runs 32 unchanged upstream programs through the production CLI and prints PASS/FAIL
# (exit 0 and status completed). A fast regression screen only; the maintained harness tools/rustc_checkout_runtime.py is the acceptance route.
# Usage: tools/validation/regress_retained.sh [binary]
BIN=${1:-$(git rev-parse --show-toplevel)/target/debug/lrr}
ROOT=$(git rev-parse --show-toplevel)/fixtures/upstream/rustc-checkout/tests/ui
cd "$(git rev-parse --show-toplevel)"
progs="enum-discriminant/repr128 enum-discriminant/discriminant_size enum-discriminant/discriminant_value associated-types/assoc-type-in-struct-constructor generics/generic-derived-type structs-enums/struct-aliases type-alias/static-method-type-alias-11047 expr/early-return-in-binop traits/bound/multiple generics/generic-extern-mangle associated-types/ret-associated-type associated-types/assoc-type-add-output-via-operator-syntax binop/add-ref-operand-method-lookup numbers-arithmetic/numeric-method-autoexport"
for u in align backcomp const-codegen const-eval-field derive-rpass drop-assign drop inherent-method macro manuallydrop-rpass modification nodrop overwrite packed pat-refutability trait-impl transmute; do progs="$progs union/union-$u"; done
progs="$progs union/union_destructure"
fail=0
for p in $progs; do
  f=$ROOT/$p.rs
  ed=$(grep -o "//@ edition: *[0-9]*" $f | grep -o "[0-9]*$" | head -1); ed=${ed:-2015}
  out=$(LRR_EMBEDDED_FACADE=1 timeout 300 $BIN --edition $ed run $f 2>&1); rc=$?
  if [ $rc -eq 0 ] && echo "$out" | grep -q '"status":"completed"'; then echo "PASS $p"; else echo "FAIL $p rc=$rc $(echo "$out" | head -1 | cut -c1-140)"; fail=1; fi
done
exit $fail
