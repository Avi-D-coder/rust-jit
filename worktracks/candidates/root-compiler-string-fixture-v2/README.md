# Existing private receipt fixture initialization

V103 compilation exposed an existing test-only private receipt initializer that needs the new generated flag. Set it to false because this control reproduces a compiler runtime assertion operand, whose destination remains a runtime scalar slot. The generated node-value receipt is separately admitted by a retained sealed plan. No production, upstream, authority or assertion changes. Validate this exact one-line follow-on with the sealed V103 repair; the failed compilation remains recorded.
