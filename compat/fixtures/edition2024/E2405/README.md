# E2405 native counterpart and relaxed source

main.rs is the valid native counterpart: skipped_bool would print a forbidden
marker if evaluated. relaxed-main.rs retains the required undefined cold call.
rustc rejection of relaxed-main.rs is not a failing native-pass oracle and does
not invalidate the relaxed LRR requirement. Full LRR preparation/evaluation-zero
assertions for the marker remain pending. No native counterpart result is an LRR
fixture pass.
