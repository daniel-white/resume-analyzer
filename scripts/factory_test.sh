#!/usr/bin/env bash
# Exercises the factory against temporary repositories. No model calls.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
factory=$root/scripts/factory.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/factory-test.XXXX")
trap 'rm -rf "$tmp"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

ok() {
  echo "ok: $*"
}

new_repo() {
  local dir=$tmp/repo-$1
  git init -q -b main "$dir"
  git -C "$dir" config user.email "factory@example.com"
  git -C "$dir" config user.name "Factory"
  git -C "$dir" config commit.gpgsign false
  printf 'seed\n' >"$dir/README"
  printf '%s\n' '.factory-state/' 'agent.sh' 'gate.sh' 'count' >"$dir/.gitignore"
  git -C "$dir" add README .gitignore
  git -C "$dir" commit -q -m seed
  printf '%s\n' "$dir"
}

seal() {
  local dir=$1
  git -C "$dir" add factory
  git -C "$dir" commit -q -m harness
}

write_waves() {
  local dir=$1
  shift
  mkdir -p "$dir/factory"
  printf '%s\n' "$@" >"$dir/factory/waves.tsv"
}

run_factory() {
  local dir=$1
  shift
  (
    cd "$dir"
    FACTORY_STATE=$dir/.factory-state \
      FACTORY_WAVES=$dir/factory/waves.tsv \
      FACTORY_CHECK=${FACTORY_CHECK:-$dir/gate.sh} \
      FACTORY_AGENT_CMD=${FACTORY_AGENT_CMD:-$dir/agent.sh} \
      FACTORY_MAX_ATTEMPTS=${FACTORY_MAX_ATTEMPTS:-3} \
      bash "$factory" "$@"
  )
}

# A passing gate once src/wave/alpha.txt contains ok.
install_gate() {
  local dir=$1
  cat >"$dir/gate.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
grep -q ok src/wave/alpha.txt
EOF
}

install_commit_agent() {
  local dir=$1
  cat >"$dir/agent.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p src/wave
printf 'ok\n' > src/wave/alpha.txt
git add src/wave/alpha.txt
git commit -q -m alpha
EOF
  chmod +x "$dir/agent.sh"
}

basic_waves() {
  printf '%s\n' $'1\talpha\tfactory/alpha\t-\tsrc/wave\tsrc/wave/alpha.txt~ok\talpha exists'
}

echo "catalog"
(
  cd "$root"
  FACTORY_STATE=$tmp/real-state bash "$factory" doctor
) >/dev/null
status=$(
  cd "$root"
  FACTORY_STATE=$tmp/real-state bash "$factory" status
)
grep -q $'^wave\tbootstrap\tdone\t' <<<"$status" || fail "bootstrap not done"
grep -q $'^wave\tspine\tready\t' <<<"$status" || fail "spine not ready"
grep -q $'^wave\tui\twaiting\t' <<<"$status" || fail "ui should wait"
grep -q $'^next\tspine$' <<<"$status" || fail "next should be spine"
if ! (
  cd "$root"
  FACTORY_STATE=$tmp/real-state bash "$factory" plan plugin north
) >/dev/null 2>"$tmp/plan.err"; then
  grep -q 'sibling' "$tmp/plan.err" || fail "plugin plan should wait for the fleet"
else
  fail "plugin plan should fail before the fleet lands"
fi
prompt=$(
  cd "$root"
  FACTORY_STATE=$tmp/real-state bash "$factory" plan spine
)
grep -q 'api/spine' <<<"$prompt" || fail "spine prompt missing branch"
grep -q 'docs/contracts.md' <<<"$prompt" || fail "spine prompt missing contracts"
ok "catalog"

echo "act merges a green wave"
repo=$(new_repo merge)
write_waves "$repo" "$(basic_waves)"
install_gate "$repo"
install_commit_agent "$repo"
seal "$repo"
run_factory "$repo" act alpha
grep -q ok "$repo/src/wave/alpha.txt" || fail "sentinel missing on main"
second=$(run_factory "$repo" act alpha)
grep -q 'already done' <<<"$second" || fail "second act should no-op"
ok "merge"

echo "red gate heals on the next attempt"
repo=$(new_repo heal)
write_waves "$repo" "$(basic_waves)"
install_gate "$repo"
cat >"$repo/agent.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p src/wave
if grep -q 'Gate failed' "$1"; then
  printf 'ok\n' > src/wave/alpha.txt
else
  printf 'bad\n' > src/wave/alpha.txt
fi
git add src/wave/alpha.txt
git commit -q -m alpha
EOF
chmod +x "$repo/agent.sh"
seal "$repo"
run_factory "$repo" act alpha
grep -q ok "$repo/src/wave/alpha.txt" || fail "heal did not land"
[[ "$(git -C "$repo" rev-list --count main..factory/alpha 2>/dev/null || echo 0)" == "0" ]] || true
git -C "$repo" merge-base --is-ancestor factory/alpha main || fail "branch not merged"
ok "heal"

echo "out of bounds is reverted before merge"
repo=$(new_repo bounds)
write_waves "$repo" "$(basic_waves)"
install_gate "$repo"
cat >"$repo/agent.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p src/wave evil
printf 'ok\n' > src/wave/alpha.txt
printf 'x\n' > evil/nope.txt
git add src evil
git commit -q -m both
EOF
chmod +x "$repo/agent.sh"
seal "$repo"
run_factory "$repo" act alpha
[[ -f "$repo/src/wave/alpha.txt" ]] || fail "allowed file missing"
[[ ! -e "$repo/evil/nope.txt" ]] || fail "out-of-bounds file merged"
ok "bounds"

echo "toolchain failure does not retry"
repo=$(new_repo infra)
write_waves "$repo" "$(basic_waves)"
cat >"$repo/gate.sh" <<'EOF'
#!/usr/bin/env bash
echo "cargo: command not found"
exit 127
EOF
echo 0 >"$repo/count"
cat >"$repo/agent.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
n=\$(cat "$repo/count")
echo \$((n + 1)) > "$repo/count"
mkdir -p src/wave
printf 'ok\n' > src/wave/alpha.txt
git add src/wave/alpha.txt
git commit -q -m alpha
EOF
chmod +x "$repo/agent.sh"
seal "$repo"
if run_factory "$repo" act alpha >"$tmp/infra.out" 2>"$tmp/infra.err"; then
  fail "infra should stop the run"
fi
grep -q 'toolchain' "$tmp/infra.err" || fail "infra reason missing"
[[ "$(cat "$repo/count")" == "1" ]] || fail "infra retried the agent"
[[ ! -f "$repo/src/wave/alpha.txt" ]] || fail "infra merged"
ok "infra"

echo "attempt cap leaves main untouched"
repo=$(new_repo cap)
write_waves "$repo" "$(basic_waves)"
install_gate "$repo"
echo 0 >"$repo/count"
cat >"$repo/agent.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
n=\$(cat "$repo/count")
echo \$((n + 1)) > "$repo/count"
mkdir -p src/wave
printf 'bad\n' > src/wave/alpha.txt
git add src/wave/alpha.txt
git commit -q -m bad
EOF
chmod +x "$repo/agent.sh"
seal "$repo"
if FACTORY_MAX_ATTEMPTS=2 run_factory "$repo" act alpha >"$tmp/cap.out" 2>"$tmp/cap.err"; then
  fail "cap should stop"
fi
[[ "$(cat "$repo/count")" == "2" ]] || fail "cap count $(cat "$repo/count")"
[[ ! -f "$repo/src/wave/alpha.txt" ]] || fail "cap merged"
ok "cap"

echo "rebase onto a moved main"
repo=$(new_repo rebase)
write_waves "$repo" "$(basic_waves)"
install_gate "$repo"
cat >"$repo/agent.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
primary=$(git worktree list --porcelain | awk '/^worktree / { print $2; exit }')
printf 'moved\n' >"$primary/moved.txt"
git -C "$primary" add moved.txt
git -C "$primary" commit -q -m "main moved"
mkdir -p src/wave
printf 'ok\n' > src/wave/alpha.txt
git add src/wave/alpha.txt
git commit -q -m alpha
EOF
chmod +x "$repo/agent.sh"
seal "$repo"
run_factory "$repo" act alpha
[[ -n "$(git -C "$repo" log --format=%s --grep '^main moved$')" ]] || fail "main commit lost"
grep -q ok "$repo/src/wave/alpha.txt" || fail "rebase dropped the wave"
ok "rebase"

echo "dirty main parks a specific path and leaves a coarse one"
repo=$(new_repo park)
write_waves "$repo" $'1\talpha\tfactory/alpha\t-\tscripts,src/keep\tsrc/keep/a.txt~ok\tkept'
mkdir -p "$repo/scripts" "$repo/src/keep"
seal "$repo"
printf 'nope\n' >"$repo/scripts/a.sh"
printf 'ok\n' >"$repo/src/keep/a.txt"
cat >"$repo/gate.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
grep -q ok src/keep/a.txt
EOF
cat >"$repo/agent.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
git add src/keep/a.txt
git commit -q -m keep
EOF
chmod +x "$repo/agent.sh"
if run_factory "$repo" act alpha >"$tmp/park.out" 2>"$tmp/park.err"; then
  fail "coarse dirt should block act"
fi
[[ -f "$repo/scripts/a.sh" ]] || fail "coarse file was removed from main"
[[ -f "$repo/.worktrees/factory-alpha/src/keep/a.txt" ]] || fail "specific file was not parked"
grep -q ok "$repo/.worktrees/factory-alpha/src/keep/a.txt" || fail "parked contents differ"
ok "park"

echo "unowned dirt blocks and moves nothing"
repo=$(new_repo unowned)
write_waves "$repo" "$(basic_waves)"
seal "$repo"
printf 'note\n' >"$repo/notes.txt"
mkdir -p "$repo/src/wave"
printf 'ok\n' >"$repo/src/wave/alpha.txt"
if run_factory "$repo" heal >"$tmp/un.out" 2>"$tmp/un.err"; then
  fail "unowned heal should fail"
fi
[[ -f "$repo/notes.txt" ]] || fail "unowned file removed"
[[ -f "$repo/src/wave/alpha.txt" ]] || fail "owned file moved despite unowned dirt"
ok "unowned"

echo "stale lock is cleared"
repo=$(new_repo lock)
write_waves "$repo" "$(basic_waves)"
install_gate "$repo"
install_commit_agent "$repo"
seal "$repo"
mkdir -p "$repo/.factory-state/lock"
echo 999999 >"$repo/.factory-state/lock/pid"
if kill -0 999999 2>/dev/null; then
  fail "test pid 999999 is alive"
fi
run_factory "$repo" act alpha
grep -q ok "$repo/src/wave/alpha.txt" || fail "stale lock blocked act"
ok "lock"

echo "plugin wave follows the skill and waits for a sibling"
repo=$(new_repo plugin)
mkdir -p "$repo/crates/plugins/src"
printf 'greenline\n' >"$repo/crates/plugins/src/greenline.rs"
git -C "$repo" add crates
git -C "$repo" commit -q -m sibling
write_waves "$repo" $'4\tfleet\tplugins/fleet\t-\tcrates/plugins\tcrates/plugins/src/greenline.rs~greenline\tfleet exists'
seal "$repo"
cat >"$repo/gate.sh" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat >"$repo/agent.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
grep -q ats-plugin "$1"
mkdir -p crates/plugins/src
printf 'fn north() {}\n' > crates/plugins/src/north.rs
printf 'mod north;\n' > crates/plugins/src/lib.rs
git add crates/plugins
git commit -q -m north
EOF
chmod +x "$repo/agent.sh"
run_factory "$repo" act plugin north
grep -q 'fn north' "$repo/crates/plugins/src/north.rs" || fail "plugin missing"
grep -q 'mod north' "$repo/crates/plugins/src/lib.rs" || fail "plugin not registered"
ok "plugin"

echo "a cycle in the catalog is rejected"
repo=$(new_repo cycle)
write_waves "$repo" \
  $'1\ta\tfactory/a\tb\tsrc/a\tsrc/a\ta' \
  $'2\tb\tfactory/b\ta\tsrc/b\tsrc/b\tb'
if run_factory "$repo" doctor >"$tmp/cycle.out" 2>"$tmp/cycle.err"; then
  fail "cycle should fail doctor"
fi
grep -q cycle "$tmp/cycle.err" || fail "cycle not reported"
ok "cycle"

echo "factory tests passed"
