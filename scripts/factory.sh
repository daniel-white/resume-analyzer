#!/usr/bin/env bash
# Act on one factory wave, then heal the same worktree until the gate is green.
# Merges to main only from this script. Agents do not merge.
#
#   factory.sh status
#   factory.sh doctor
#   factory.sh plan [wave|plugin <id>]
#   factory.sh heal [--dry-run]
#   factory.sh act  [wave|plugin <id>]
#   factory.sh run
#
# FACTORY_WAVES       catalog (default factory/waves.tsv)
# FACTORY_STATE       progress directory (default $GIT_COMMON_DIR/factory-state)
# FACTORY_CHECK       gate to run inside the worktree (default scripts/check.sh)
# FACTORY_AGENT_CMD   program that receives the prompt file (default: opencode run)
# FACTORY_MAX_ATTEMPTS  code attempts before stopping (default 3)
set -euo pipefail

export GIT_EDITOR="${GIT_EDITOR:-true}"
export GIT_MERGE_AUTOEDIT=no

die() {
  echo "factory: $*" >&2
  exit 1
}

invoke_root=$(git rev-parse --show-toplevel)
primary=$(git -C "$invoke_root" worktree list --porcelain | awk '/^worktree / { print $2; exit }')
[[ -n "$primary" ]] || die "no primary worktree"

if [[ -z "${FACTORY_WAVES:-}" ]]; then
  if [[ -f "$invoke_root/factory/waves.tsv" ]]; then
    FACTORY_WAVES=$invoke_root/factory/waves.tsv
  else
    FACTORY_WAVES=$primary/factory/waves.tsv
  fi
fi

if [[ -z "${FACTORY_STATE:-}" ]]; then
  common=$(git -C "$invoke_root" rev-parse --git-common-dir)
  if [[ "$common" != /* ]]; then
    common=$invoke_root/$common
  fi
  FACTORY_STATE=$common/factory-state
fi

max_attempts=${FACTORY_MAX_ATTEMPTS:-3}
dry_run=0
lock_dir=""

release_lock() {
  if [[ -n "$lock_dir" && -d "$lock_dir" ]]; then
    rm -rf "$lock_dir"
  fi
}

acquire_lock() {
  mkdir -p "$FACTORY_STATE"
  lock_dir=$FACTORY_STATE/lock
  while true; do
    if mkdir "$lock_dir" 2>/dev/null; then
      echo $$ >"$lock_dir/pid"
      trap release_lock EXIT
      return
    fi
    local pid=""
    if [[ -f "$lock_dir/pid" ]]; then
      pid=$(cat "$lock_dir/pid")
    fi
    if [[ -z "$pid" ]] || ! kill -0 "$pid" 2>/dev/null; then
      rm -rf "$lock_dir"
      continue
    fi
    die "already acting (pid $pid)"
  done
}

declare -a wave_ids=()
declare -A wave_order=()
declare -A wave_branch=()
declare -A wave_needs=()
declare -A wave_allow=()
declare -A wave_done_spec=()
declare -A wave_promise=()
declare -A color=()

load_waves() {
  [[ -f "$FACTORY_WAVES" ]] || die "no wave catalog at $FACTORY_WAVES"
  wave_ids=()
  wave_order=()
  wave_branch=()
  wave_needs=()
  wave_allow=()
  wave_done_spec=()
  wave_promise=()
  local line tabs order id branch needs allow done promise
  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    tabs=${line//[!$'\t']/}
    [[ ${#tabs} -ge 6 ]] || die "wave row needs 7 fields: $line"
    IFS=$'\t' read -r order id branch needs allow done promise <<<"$line"
    [[ -n "$id" && -n "$branch" && -n "$needs" && -n "$allow" && -n "$done" && -n "$promise" ]] || die "empty field in $id"
    if [[ -n "${wave_branch[$id]:-}" ]]; then
      die "duplicate wave id $id"
    fi
    wave_ids+=("$id")
    wave_order[$id]=$order
    wave_branch[$id]=$branch
    wave_needs[$id]=$needs
    wave_allow[$id]=$allow
    wave_done_spec[$id]=$done
    wave_promise[$id]=$promise
  done <"$FACTORY_WAVES"
}

known_wave() {
  [[ -n "${wave_branch[$1]:-}" ]]
}

comma_fields() {
  local raw=$1 part
  local IFS=','
  # shellcheck disable=SC2206
  local parts=($raw)
  for part in "${parts[@]}"; do
    printf '%s\n' "$part"
  done
}

path_on_tree() {
  local tree=$1 entry=$2 path substr
  if [[ "$entry" == *~* ]]; then
    path=${entry%%~*}
    substr=${entry#*~}
    git -C "$primary" grep -F -q -e "$substr" "$tree" -- "$path"
  else
    git -C "$primary" cat-file -e "$tree:$entry" 2>/dev/null
  fi
}

wave_done() {
  local id=$1 entry
  while IFS= read -r entry; do
    [[ -n "$entry" ]] || continue
    path_on_tree main "$entry" || return 1
  done < <(comma_fields "${wave_done_spec[$id]}")
}

wave_ready() {
  local id=$1 dep
  [[ "${wave_needs[$id]}" == "-" ]] && return 0
  while IFS= read -r dep; do
    [[ -n "$dep" ]] || continue
    known_wave "$dep" || return 1
    wave_done "$dep" || return 1
  done < <(comma_fields "${wave_needs[$id]}")
}

doctor() {
  load_waves
  [[ ${#wave_ids[@]} -gt 0 ]] || die "catalog is empty"
  declare -A seen_branch=()
  local id dep prefix
  for id in "${wave_ids[@]}"; do
    if [[ -n "${seen_branch[${wave_branch[$id]}]:-}" ]]; then
      die "duplicate branch ${wave_branch[$id]}"
    fi
    seen_branch[${wave_branch[$id]}]=1
    [[ "${wave_order[$id]}" =~ ^[0-9]+$ ]] || die "$id order is not a number"
    if [[ "${wave_needs[$id]}" != "-" ]]; then
      while IFS= read -r dep; do
        [[ -n "$dep" ]] || continue
        known_wave "$dep" || die "$id needs unknown wave $dep"
        [[ "$dep" != "$id" ]] || die "$id needs itself"
      done < <(comma_fields "${wave_needs[$id]}")
    fi
    local prefixes=0
    while IFS= read -r prefix; do
      [[ -n "$prefix" ]] || continue
      prefixes=$((prefixes + 1))
      [[ "$prefix" != /* && "$prefix" != *..* && "$prefix" != *","* ]] || die "$id allow prefix is unsafe: $prefix"
    done < <(comma_fields "${wave_allow[$id]}")
    [[ "$prefixes" -gt 0 ]] || die "$id has an empty allowlist"
  done

  color=()
  visit() {
    local cur=$1 dep
    case "${color[$cur]:-white}" in
      gray) die "cycle at $cur" ;;
      black) return 0 ;;
    esac
    color[$cur]=gray
    if [[ "${wave_needs[$cur]}" != "-" ]]; then
      while IFS= read -r dep; do
        [[ -n "$dep" ]] || continue
        visit "$dep"
      done < <(comma_fields "${wave_needs[$cur]}")
    fi
    color[$cur]=black
  }
  for id in "${wave_ids[@]}"; do
    visit "$id"
  done
}

slug_ok() {
  local slug=$1
  [[ "$slug" =~ ^[a-z][a-z0-9_]{1,31}$ ]] || return 1
  case "$slug" in
    lib | mod | main | tests | plugin) return 1 ;;
  esac
  local id
  for id in "${wave_ids[@]}"; do
    [[ "$id" == "plugin-$slug" || "${wave_branch[$id]}" == "plugins/$slug" ]] && return 1
  done
  if git -C "$primary" cat-file -e "main:crates/plugins/src/$slug.rs" 2>/dev/null; then
    return 1
  fi
}

ensure_plugin_wave() {
  local slug=$1
  known_wave fleet || die "catalog has no fleet wave"
  if ! wave_done fleet; then
    die "a new plugin waits until plugins/fleet has landed a sibling to copy"
  fi
  slug_ok "$slug" || die "plugin id '$slug' is not a new rust module name"
  local id="plugin-$slug"
  if known_wave "$id"; then
    return 0
  fi
  wave_ids+=("$id")
  wave_order[$id]=50
  wave_branch[$id]="plugins/$slug"
  wave_needs[$id]=fleet
  wave_allow[$id]="crates/plugins,web/src/mock/fleet.ts"
  wave_done_spec[$id]="crates/plugins/src/$slug.rs~$slug,crates/plugins/src/lib.rs~mod $slug"
  wave_promise[$id]="Add one simulated ATS with id ${slug}. Read .opencode/skills/ats-plugin/SKILL.md and follow it. Copy a sibling. Do not invent a second verdict shape."
}

next_ready() {
  local id
  for id in "${wave_ids[@]}"; do
    if ! wave_done "$id" && wave_ready "$id"; then
      printf '%s\n' "$id"
      return 0
    fi
  done
  return 1
}

matches_prefix() {
  local path=$1 prefix=$2
  [[ "$path" == "$prefix" || "$path" == "$prefix"/* ]]
}

declare -a dirty_paths=()
declare -a owned_paths=()
declare -a owned_waves=()
declare -a coarse_paths=()
declare -a unowned_paths=()

collect_dirty() {
  dirty_paths=()
  local entry status path
  while IFS= read -r -d '' entry; do
    [[ -z "$entry" ]] && continue
    status=${entry:0:2}
    case "$status" in
      R* | C*) die "rename on main is not relocated (${entry:3})" ;;
    esac
    path=${entry:3}
    [[ -n "$path" ]] || continue
    dirty_paths+=("$path")
  done < <(git -C "$primary" status --porcelain=v1 -z --untracked-files=all)
}

consider_path() {
  local path=$1 id=$2 allow=$3 order=$4 prefix len
  declare -n slot_id=best_id
  declare -n slot_len=best_len
  declare -n slot_order=best_order
  declare -n slot_prefix=best_prefix
  while IFS= read -r prefix; do
    [[ -n "$prefix" ]] || continue
    matches_prefix "$path" "$prefix" || continue
    len=${#prefix}
    if ((len > slot_len)) || { ((len == slot_len)) && ((order < slot_order)); }; then
      slot_len=$len
      slot_order=$order
      slot_id=$id
      slot_prefix=$prefix
    fi
  done < <(comma_fields "$allow")
}

classify_dirty() {
  owned_paths=()
  owned_waves=()
  coarse_paths=()
  unowned_paths=()
  collect_dirty
  local path id prefix best_id best_len best_order best_prefix slug
  declare -A extra_allow=()
  declare -A extra_order=()
  local -a extra_ids=()
  if known_wave fleet && wave_done fleet; then
    for path in "${dirty_paths[@]+"${dirty_paths[@]}"}"; do
      [[ "$path" =~ ^crates/plugins/src/([a-z][a-z0-9_]{1,31})\.rs$ ]] || continue
      slug="${BASH_REMATCH[1]}"
      case "$slug" in
        lib | mod | main | tests | plugin) continue ;;
      esac
      slug_ok "$slug" || continue
      id="plugin-$slug"
      if [[ -z "${extra_allow[$id]:-}" ]]; then
        extra_ids+=("$id")
        extra_allow[$id]="crates/plugins,web/src/mock/fleet.ts"
        extra_order[$id]=50
      fi
    done
  fi
  if [[ ${#extra_ids[@]} -gt 1 ]]; then
    die "main has more than one new plugin (${extra_ids[*]}); park one at a time"
  fi
  for path in "${dirty_paths[@]+"${dirty_paths[@]}"}"; do
    best_id=""
    best_len=-1
    best_order=99999
    best_prefix=""
    for id in "${wave_ids[@]}"; do
      wave_done "$id" && continue
      consider_path "$path" "$id" "${wave_allow[$id]}" "${wave_order[$id]}"
    done
    for id in "${extra_ids[@]+"${extra_ids[@]}"}"; do
      consider_path "$path" "$id" "${extra_allow[$id]}" "${extra_order[$id]}"
    done
    if [[ -z "$best_id" ]]; then
      unowned_paths+=("$path")
    elif [[ "$best_prefix" != */* ]]; then
      coarse_paths+=("$path")
    else
      owned_paths+=("$path")
      owned_waves+=("$best_id")
    fi
  done
}

print_dirty() {
  local i path id
  for i in "${!owned_paths[@]}"; do
    printf 'dirty\t%s\t%s\n' "${owned_waves[$i]}" "${owned_paths[$i]}"
  done
  for path in "${coarse_paths[@]+"${coarse_paths[@]}"}"; do
    printf 'coarse\t%s\n' "$path"
  done
  for path in "${unowned_paths[@]+"${unowned_paths[@]}"}"; do
    printf 'unowned\t%s\n' "$path"
  done
}

worktree_for() {
  local id=$1
  local branch=${wave_branch[$id]}
  printf '%s\n' "$primary/.worktrees/${branch//\//-}"
}

ensure_worktree() {
  local id=$1
  local dest branch
  dest=$(worktree_for "$id")
  branch=${wave_branch[$id]}
  if [[ -d "$dest" ]]; then
    local current
    current=$(git -C "$dest" branch --show-current)
    [[ "$current" == "$branch" ]] || die "worktree $dest is on $current, not $branch"
    return 0
  fi
  mkdir -p "$primary/.worktrees"
  if git -C "$primary" show-ref --verify --quiet "refs/heads/$branch"; then
    git -C "$primary" worktree add "$dest" "$branch"
  else
    git -C "$primary" worktree add -b "$branch" "$dest" main
  fi
}

on_main() {
  [[ "$(git -C "$primary" branch --show-current)" == "main" ]]
}

require_main() {
  on_main || die "primary worktree is on $(git -C "$primary" branch --show-current), not main"
}

move_owned() {
  local i path id dest src
  declare -A made=()
  for i in "${!owned_paths[@]}"; do
    id=${owned_waves[$i]}
    if [[ "$id" == plugin-* ]] && ! known_wave "$id"; then
      ensure_plugin_wave "${id#plugin-}"
    fi
    if [[ -z "${made[$id]:-}" ]]; then
      ensure_worktree "$id"
      made[$id]=1
    fi
  done
  for i in "${!owned_paths[@]}"; do
    path=${owned_paths[$i]}
    id=${owned_waves[$i]}
    dest=$(worktree_for "$id")
    src=$primary/$path
    local target=$dest/$path
    if [[ -e "$src" ]]; then
      if [[ -e "$target" ]] && ! cmp -s "$src" "$target"; then
        die "conflict parking $path into $dest"
      fi
      mkdir -p "$(dirname "$target")"
      cp -p "$src" "$target"
      cmp -s "$src" "$target" || die "copy failed for $path"
    elif [[ -e "$target" ]]; then
      rm -f "$target"
    fi
    if git -C "$primary" ls-files --error-unmatch -- "$path" >/dev/null 2>&1; then
      git -C "$primary" restore --source=HEAD --staged --worktree -- "$path"
    else
      rm -f "$src"
    fi
  done
}

heal_main() {
  require_main
  classify_dirty
  if [[ ${#unowned_paths[@]} -gt 0 ]]; then
    print_dirty >&2
    die "main has files the factory cannot attribute"
  fi
  if [[ "$dry_run" == 1 ]]; then
    print_dirty
    return 0
  fi
  if [[ ${#owned_paths[@]} -eq 0 ]]; then
    print_dirty
    return 0
  fi
  move_owned
  print_dirty
}

require_clean_main() {
  if [[ -n "$(git -C "$primary" status --porcelain)" ]]; then
    die "main is still dirty; the factory will not act"
  fi
}

write_prompt() {
  local id=$1 dest=$2 attempt=$3
  local prompt=$FACTORY_STATE/$id/prompt.md
  mkdir -p "$FACTORY_STATE/$id"
  local sentinels="" entry path substr
  while IFS= read -r entry; do
    [[ -n "$entry" ]] || continue
    if [[ "$entry" == *~* ]]; then
      path=${entry%%~*}
      substr=${entry#*~}
      sentinels+="- ${path} must contain: ${substr}"$'\n'
    else
      sentinels+="- ${entry} must exist"$'\n'
    fi
  done < <(comma_fields "${wave_done_spec[$id]}")
  {
    echo "You are the factory agent for one wave. Work only in this worktree."
    echo
    echo "Branch: ${wave_branch[$id]}"
    echo "Worktree: $dest"
    echo "Attempt: $attempt of $max_attempts"
    echo "You may change only: ${wave_allow[$id]//,/, }"
    echo "Anything else is reverted. Do not put it back."
    echo "Contracts: docs/contracts.md (ParseInput, Events, Callback, HTTP, Verdict). Do not invent a second shape beside them."
    echo "Promise: ${wave_promise[$id]}"
    echo "These must be true on this branch before the gate can pass:"
    printf '%s' "$sentinels"
    echo "Done means scripts/check.sh exits 0, the promise holds, and the paths above are committed."
    echo "Size: one branch, reviewable in one sitting. If the work needs more, stop and propose a split."
    echo "Commit on this branch. Do not merge. Do not push. Do not create a worktree."
    if [[ "$id" == fleet || "$id" == plugin-* ]]; then
      echo
      echo "Read .opencode/skills/ats-plugin/SKILL.md and follow it, including references/factor-worksheet.md."
      echo "The skill id is ats-plugin."
    fi
    if [[ -s "$FACTORY_STATE/$id/notes" ]]; then
      echo
      echo "Previous attempt:"
      cat "$FACTORY_STATE/$id/notes"
    fi
  } >"$prompt"
  printf '%s\n' "$prompt"
}

changed_files() {
  local wt=$1
  {
    git -C "$wt" diff --name-only main...HEAD
    git -C "$wt" diff --name-only
    git -C "$wt" diff --name-only --cached
    git -C "$wt" ls-files --others --exclude-standard
  } | sort -u
}

outside_allowlist() {
  local wt=$1 id=$2 path
  while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    local ok=0 prefix
    while IFS= read -r prefix; do
      [[ -n "$prefix" ]] || continue
      if matches_prefix "$path" "$prefix"; then
        ok=1
        break
      fi
    done < <(comma_fields "${wave_allow[$id]}")
    if [[ "$ok" == 0 ]]; then
      printf '%s\n' "$path"
    fi
  done < <(changed_files "$wt")
}

revert_outside() {
  local wt=$1 id=$2
  local -a bad=()
  local path
  while IFS= read -r path; do
    [[ -n "$path" ]] && bad+=("$path")
  done < <(outside_allowlist "$wt" "$id")
  [[ ${#bad[@]} -eq 0 ]] && return 0
  for path in "${bad[@]}"; do
    if git -C "$wt" cat-file -e "HEAD:$path" 2>/dev/null || git -C "$wt" ls-files --error-unmatch -- "$path" >/dev/null 2>&1; then
      if git -C "$wt" cat-file -e "main:$path" 2>/dev/null; then
        git -C "$wt" checkout main -- "$path"
      else
        git -C "$wt" rm -f --ignore-unmatch -- "$path" >/dev/null 2>&1 || true
        rm -rf "$wt/$path"
      fi
    else
      rm -rf "$wt/$path"
    fi
  done
  if ! git -C "$wt" diff --cached --quiet; then
    git -C "$wt" commit -q -m "factory: revert paths outside the wave allowlist"
  fi
  printf '%s\n' "${bad[@]}"
}

run_agent() {
  local id=$1 prompt=$2 wt=$3
  if [[ -n "${FACTORY_AGENT_CMD:-}" ]]; then
    (cd "$wt" && "$FACTORY_AGENT_CMD" "$prompt")
    return
  fi
  local session=""
  if [[ -f "$FACTORY_STATE/$id/session" ]]; then
    session=$(cat "$FACTORY_STATE/$id/session")
  fi
  local -a args=(run --auto --format json --title "factory $id")
  if [[ -n "$session" ]]; then
    args+=(--session "$session")
  fi
  local out code
  set +e
  out=$(cd "$wt" && opencode "${args[@]}" "$(cat "$prompt")")
  code=$?
  set -e
  mkdir -p "$FACTORY_STATE/$id"
  printf '%s\n' "$out" >>"$FACTORY_STATE/$id/agent.log"
  local found
  found=$(printf '%s\n' "$out" | grep -oE 'ses_[A-Za-z0-9]+' | head -n 1 || true)
  if [[ -n "$found" ]]; then
    printf '%s\n' "$found" >"$FACTORY_STATE/$id/session"
  fi
  return "$code"
}

is_infra() {
  local code=$1 log=$2
  if [[ "$code" -eq 127 ]]; then
    return 0
  fi
  if [[ -f "$log" ]] && grep -Eq 'command not found|failed to download|Could not resolve host|Network is unreachable|Temporary failure in name resolution' "$log"; then
    return 0
  fi
  return 1
}

run_gate() {
  local wt=$1 log=$2
  local check
  if [[ -n "${FACTORY_CHECK:-}" ]]; then
    check=$FACTORY_CHECK
  elif [[ -f "$wt/scripts/check.sh" ]]; then
    check=$wt/scripts/check.sh
  else
    check=$primary/scripts/check.sh
  fi
  local code
  set +e
  if [[ -n "${FACTORY_CHECK:-}" ]]; then
    (cd "$wt" && bash "$check") >"$log" 2>&1
  else
    (cd "$wt" && bash "$check") >"$log" 2>&1
  fi
  code=$?
  set -e
  printf '%s\n' "$code"
}

branch_has_sentinels() {
  local wt=$1 id=$2 entry
  while IFS= read -r entry; do
    [[ -n "$entry" ]] || continue
    if [[ "$entry" == *~* ]]; then
      local path=${entry%%~*} substr=${entry#*~}
      git -C "$wt" grep -F -q -e "$substr" HEAD -- "$path" || return 1
    else
      git -C "$wt" cat-file -e "HEAD:$entry" 2>/dev/null || return 1
    fi
  done < <(comma_fields "${wave_done_spec[$id]}")
}

rebase_onto_main() {
  local wt=$1
  local code
  set +e
  git -C "$wt" rebase main >"$FACTORY_STATE/rebase.log" 2>&1
  code=$?
  set -e
  if [[ "$code" -ne 0 ]]; then
    git -C "$wt" rebase --abort >/dev/null 2>&1 || true
    return 1
  fi
  return 0
}

act_wave() {
  local id=$1
  known_wave "$id" || die "unknown wave $id"
  if wave_done "$id"; then
    echo "factory: $id is already done"
    return 0
  fi
  wave_ready "$id" || die "$id is waiting on ${wave_needs[$id]}"
  require_main
  heal_main
  require_clean_main
  ensure_worktree "$id"
  local dest notes log attempt ahead dirty reverted
  dest=$(worktree_for "$id")
  mkdir -p "$FACTORY_STATE/$id"
  notes=$FACTORY_STATE/$id/notes
  : >"$notes"

  for ((attempt = 1; attempt <= max_attempts; attempt++)); do
    echo "$attempt" >"$FACTORY_STATE/$id/attempts"
    echo "acting" >"$FACTORY_STATE/$id/status"
    local prompt
    prompt=$(write_prompt "$id" "$dest" "$attempt")
    set +e
    run_agent "$id" "$prompt" "$dest"
    local agent_code=$?
    set -e

    reverted=$(revert_outside "$dest" "$id" || true)
    if [[ -n "$(outside_allowlist "$dest" "$id")" ]]; then
      echo "blocked" >"$FACTORY_STATE/$id/status"
      die "$id still has paths outside the allowlist after revert"
    fi

    if ! rebase_onto_main "$dest"; then
      echo "blocked" >"$FACTORY_STATE/$id/status"
      die "$id rebase onto main conflicted; aborted"
    fi

    log=$FACTORY_STATE/$id/gate.log
    local gate
    gate=$(run_gate "$dest" "$log")
    if is_infra "$gate" "$log"; then
      echo "blocked" >"$FACTORY_STATE/$id/status"
      tail -n 20 "$log" >&2 || true
      die "$id gate failed in the toolchain, not the change (exit $gate)"
    fi

    dirty=$(git -C "$dest" status --porcelain)
    ahead=$(git -C "$dest" rev-list --count main..HEAD)
    if [[ "$gate" -eq 0 && -z "$dirty" && "$ahead" -gt 0 ]] && branch_has_sentinels "$dest" "$id"; then
      git -C "$primary" merge --ff-only "${wave_branch[$id]}" >/dev/null
      if [[ "$(pwd)" != "$dest" && "$(pwd)" != "$dest"/* ]]; then
        git -C "$primary" worktree remove "$dest"
      fi
      wave_done "$id" || die "$id merged but its sentinel is not on main"
      echo "merged" >"$FACTORY_STATE/$id/status"
      rm -f "$notes"
      echo "factory: merged $id (${wave_branch[$id]})"
      return 0
    fi

    {
      echo "Gate failed."
      if [[ "$gate" -ne 0 ]]; then
        echo "Exit $gate. The last lines of the gate:"
        tail -n 40 "$log" || true
      fi
      if [[ -n "$dirty" ]]; then
        echo "Uncommitted work remains. Commit it on this branch. Do not merge."
      fi
      if [[ "$ahead" -eq 0 ]]; then
        echo "The branch has no commit of its own."
      fi
      if ! branch_has_sentinels "$dest" "$id"; then
        echo "The sentinel for this wave is not committed on the branch."
      fi
      if [[ "$agent_code" -ne 0 ]]; then
        echo "Agent exited $agent_code."
      fi
      if [[ -n "$reverted" ]]; then
        echo "Reverted paths outside the allowlist:"
        printf '%s\n' "$reverted"
      fi
    } >"$notes"
    echo "factory: $id attempt $attempt did not merge"
  done

  echo "blocked" >"$FACTORY_STATE/$id/status"
  die "$id stopped after $max_attempts attempts"
}

cmd_status() {
  load_waves
  local id state needs
  for id in "${wave_ids[@]}"; do
    if wave_done "$id"; then
      state=done
    elif wave_ready "$id"; then
      state=ready
    else
      state=waiting
    fi
    if [[ "$state" == waiting ]]; then
      printf 'wave\t%s\t%s\t%s\tneeds %s\n' "$id" "$state" "${wave_branch[$id]}" "${wave_needs[$id]}"
    else
      printf 'wave\t%s\t%s\t%s\n' "$id" "$state" "${wave_branch[$id]}"
    fi
  done
  if on_main; then
    classify_dirty
    print_dirty
  fi
  local next=""
  if next=$(next_ready); then
    printf 'next\t%s\n' "$next"
  else
    printf 'next\tnone\n'
  fi
}

cmd_plan() {
  load_waves
  local id=${1:-}
  if [[ "$id" == plugin ]]; then
    [[ -n "${2:-}" ]] || die "usage: plan plugin <id>"
    ensure_plugin_wave "$2"
    id="plugin-$2"
  elif [[ -z "$id" ]]; then
    id=$(next_ready) || die "no ready wave"
  fi
  known_wave "$id" || die "unknown wave $id"
  local dest
  dest=$(worktree_for "$id")
  write_prompt "$id" "$dest" 1
  echo "factory: prompt for $id is in $FACTORY_STATE/$id/prompt.md" >&2
  cat "$FACTORY_STATE/$id/prompt.md"
}

cmd_heal() {
  acquire_lock
  load_waves
  heal_main
}

cmd_act() {
  acquire_lock
  load_waves
  local id=${1:-}
  if [[ "$id" == plugin ]]; then
    [[ -n "${2:-}" ]] || die "usage: act plugin <id>"
    ensure_plugin_wave "$2"
    act_wave "plugin-$2"
    return
  fi
  if [[ -z "$id" ]]; then
    id=$(next_ready) || die "no ready wave"
  fi
  act_wave "$id"
}

cmd_run() {
  acquire_lock
  load_waves
  local spins=0 max=$((${#wave_ids[@]} + 1)) i id
  for ((i = 0; i < max; i++)); do
    if ! id=$(next_ready); then
      echo "factory: no ready wave"
      return 0
    fi
    act_wave "$id"
    if wave_done "$id"; then
      spins=0
    else
      spins=$((spins + 1))
      [[ "$spins" -ge 2 ]] && die "$id did not finish"
    fi
  done
  die "run stopped after $max waves"
}

args=()
for arg in "$@"; do
  if [[ "$arg" == --dry-run ]]; then
    dry_run=1
  else
    args+=("$arg")
  fi
done

cmd=${args[0]:-status}
case "$cmd" in
  status) cmd_status ;;
  doctor)
    doctor
    echo "factory: catalog ok"
    ;;
  plan) cmd_plan "${args[@]:1}" ;;
  heal) cmd_heal ;;
  act) cmd_act "${args[@]:1}" ;;
  run) cmd_run ;;
  *) die "usage: factory.sh status|doctor|plan|heal|act|run" ;;
esac
