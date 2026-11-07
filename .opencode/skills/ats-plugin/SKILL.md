---
name: ATS plugin
description: Add or fix a simulated ATS plugin in the resume-analyzer fleet. Use when asked to add an ATS provider or plugin, when a factory wave is plugins/<id>, or when plugin factor lists disagree less than they should.
---

# Add an ATS plugin

The factory heals a plugin by re-running the gate on the same branch. This skill is the procedure for that attempt. It is not an agent profile. Do not merge, do not push, and do not open a worktree. The factory does that.

## Read, then stop if the contract is not real

Read in this order:

1. `docs/contracts.md`. If it still says the sections are headings only, or Verdict has no shape, stop. Do not draft `ParseInput`, the verdict, or the callback in the plugin crate.
2. `docs/plan.md`, sections "The simulated fleet" and "Event spine". Greenline, Levr, Workdaye, and Ash keep those names and biases. Do not rename them.
3. `crates/domain`. Use the plugin trait and verdict types that are defined there.
4. `crates/plugins`. Read the registry and every sibling, including how a callback is signed and where the HMAC secret comes from.
5. The two fixture resumes the fleet tests already load. If they are not there and this is not the fleet wave seeding them, stop.

## Invariants

These are not negotiable inside a plugin branch:

- Compiled in. No dynamic loading, no vendor OAuth, no trademarked name, no logo.
- A plugin does not return a verdict into the worker. Delivery is a signed `POST /api/callbacks/{plugin_id}`. Copy the sibling's delivery. The body is plugin id, run id, verdict, and factors. HMAC covers the timestamp plus the body. Reject timestamps skewed past five minutes. Ignore a replayed idempotency key.
- The HMAC secret is per plugin and comes from the same config the siblings use. Never commit a secret. If that config does not exist yet, stop and name it.
- Verdict fields are kept, dropped, a score from 0 to 100, and a factor list. No prose. No model call. One structuring call per resume already happened elsewhere.
- A table drop on a PDF stays inferred (`LayoutConfidence`). Do not report it as a fact.
- Tests assert factor labels and signs, never the headline score.
- The legend subtitle says the system is simulated and modeled on publicly described parser behavior.

## Add one plugin

Do this only when at least one sibling already exists. Otherwise stop, unless the prompt is the fleet wave seeding the four.

1. Read `references/factor-worksheet.md` and fill the table in the session. Do not commit the worksheet.
2. Choose a `snake_case` id that is a valid Rust module name. That id is the module, the callback path, and the legend id.
3. Add `crates/plugins/src/<id>.rs` by copying a sibling's shape, not its bias. Register it the same way, including `mod <id>` in `crates/plugins/src/lib.rs`.
4. Add its HMAC secret slot the way the sibling's is added.
5. Keep a single legend. If the UI reads `GET /api/plugins`, add the row there only. If the page still imports `web/src/mock/fleet.ts` and that file is what it renders, add the row there instead. Do not leave two lists once the API is what the page renders.
6. Add tests beside the sibling's tests, on the designed fixture and the plain fixture. Assert labels. The designed label set must differ from every sibling. The plain fixture must not trip the signals this plugin punishes.
7. From the worktree, run `scripts/check.sh`. Commit on the branch you were given.

If the bias needs a layout signal that is not on `ParseInput`, stop and name the signal. Do not parse resume bytes inside the plugin, and do not edit the contract on this branch.

## Seed the fleet

Only the fleet wave does this. Add Greenline, Levr, Workdaye, and Ash with the biases in `docs/plan.md`. The first plugin creates the registry, the callback delivery, the secret config, and the test layout. The other three copy that shape. On the designed fixture the four factor lists disagree. On the plain fixture they agree: high scores, little or nothing dropped.

The fleet test lives at `crates/plugins/tests/fleet.rs` and mentions `greenline`. Assert the factor lists, not the scores.

## Heal

The factory sends the failing gate output back into this same worktree. React to that output, not to a new design.

- `cargo fmt` or clippy names a line. Fix that line. Do not retune deltas in the same attempt.
- An assertion on a headline score is wrong. Replace it with the factor label.
- The designed factor list equals a sibling. Change this plugin's bias. Do not change the sibling, the fixture, or the assertion to hide the tie.
- You edited `crates/domain`, `docs/contracts.md`, or `crates/app` so the plugin would fit. Revert those edits. If the trait cannot say what you need, stop.
- The same assertion fails three times. Stop. Report the assertion, the current factor list, and the deltas you already tried.
- A path outside `crates/plugins`, and outside the one legend file when you had to touch it, is out of bounds. The factory reverts it. Do not put it back.

When the gate is green and the work is committed, stop. The factory rebases onto main and merges.
