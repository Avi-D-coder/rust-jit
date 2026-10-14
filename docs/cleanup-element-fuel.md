# T031 scalar scope cleanup element fuel — work in progress

Scope cleanup now admits each occupied temporary or binding cell before charging
one helper element unit. After the charge, it releases its current live owned
value, retires the location and clears the frame reference. Uninitialized and
moved locations also count as processed cells. Empty physical slots do not charge.
The cleanup completion counter advances only after that cell retires; the opcode
and exit counters remain separate. Fuel and cancellation use the existing meter.

A resource fault or interruption preserves unprocessed cells and the active scope.
Already completed retirements remain committed. A failed overall cleanup does not
pop that scope. The test uses actual owned strings and verifies that exhaustion
after one element retires only the last cell, while pre-requested cancellation
retires none. This does not permit retrying a terminal guest run.

This checkpoint covers the existing scalar/literal/string inventory. General
user Drop scheduling, aggregate fields, bounded resumable OpActivation chunks,
and canonical bound operation identity remain required later integrations. The
current scope inventory scan is not proof of the Section 16.7 chunk contract.
T031 original-source component is accepted under E-57-001; see the
[acceptance audit](t031-acceptance-audit.md). Full integration/release remain
incomplete; Linux acceptance remains deferred.
