# Correct byte control boundaries

V79 diagnostics identified two harness contract errors. The declaration-prefix control used an expression return helper on a let statement, selecting the enclosing block Commit rather than the next statement. Select its actual statement successor; both original declarations must still execute, and the next original assertion remains explicitly unexecuted.

The quota control omitted the real ScopeEnter preceding SetHint and preactivated its scope. Require the exact two-operation recipe, start at the scope parent, and distinguish zero-fuel refusal from metadata refusal after the completed ScopeEnter. Guest storage/type state must remain unchanged in both cases; only the earlier ScopeEnter may consume one fuel/opcode. No SetHint effects are accepted. This follows operation-level preflight rather than inventing recipe-wide rollback.

Source-only T041 harness changes, no production or upstream edits. Full-fixture and runtime acceptance remain pending.
