# Factor worksheet

Fill this in the session before editing code. Do not commit it.

Use the same base score and the same clamp as the sibling plugins. If no sibling has a base yet, stop. Do not invent a second scoring system. The fleet wave is the only time a base may be introduced, and every plugin seeded in that wave shares it.

Score is `clamp(base + sum of deltas whose signal is present, 0, 100)`. Tests do not assert that number. They assert the labels below.

| Signal | Designed | Plain | Delta | Label |
| --- | --- | --- | --- | --- |
|  | yes/no | yes/no |  |  |

- Deltas are integers. A punishment is negative. A label is short and stable, like `contact in header`.
- A signal this plugin claims to punish is `yes` on the designed fixture and `no` on the plain one. If it would also fire on the plain fixture, it is the wrong signal.
- The designed column must produce a different set of labels than every sibling. If it does not, change this bias, not the sibling and not the fixture.
- A drop that came from an inferred PDF table stays inferred. The label does not say the table was a fact.
- No row is prose. The legend subtitle is one line and lives with the registration, not in this table.
