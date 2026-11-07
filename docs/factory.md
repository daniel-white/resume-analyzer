# Factory

`scripts/factory.sh` is the actor for the waves in `docs/plan.md`. It opens a
worktree, runs one attempt, and if the gate is red it tries the same worktree
again. It merges to `main` only when `scripts/check.sh` is green, the diff is
inside the wave's allowlist, and the work is committed. A missing toolchain
stops the run. That is not a code failure, and it does not spend an attempt.

Waves are `factory/waves.tsv`. `just factory` prints the catalog. `just factory act`
runs the next ready wave. `just factory act plugin <id>` adds one simulated ATS
on `plugins/<id>` by following `.opencode/skills/ats-plugin/SKILL.md`. The skill
is the procedure. It is not an agent profile, and none is committed.

The agent is `opencode run --auto` inside the worktree. Tests and dry runs set
`FACTORY_AGENT_CMD` to a program that receives the prompt file. The factory
reverts paths outside the allowlist, rebases onto `main`, and stops on a
conflict instead of improvising one.

Files left on `main` are parked only when one not-done wave owns them by a
prefix that contains a slash. `web/src/...` can move to `web/ui`. A loose
prefix such as `scripts` or `docs` is left alone, and `act` refuses to start
while `main` is still dirty. A path the catalog cannot attribute blocks the
heal and moves nothing. A crashed run leaves a lock whose pid is dead; the
next run removes it.

`scripts/factory_test.sh` is the gate for this actor. It does not call a model.

The web half of `scripts/check.sh` runs Playwright when `web/playwright.config.ts` is on the branch. The `ui` wave may change `web/e2e`, and it is not done until those tests are on `main`.
