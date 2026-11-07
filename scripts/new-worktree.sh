#!/usr/bin/env bash
# Create .worktrees/<branch-with-slashes-as-dashes> off main.
# The caller merges; this script only checks the branch out.
set -euo pipefail

branch=${1:?usage: new-worktree.sh <branch>}
root=$(git rev-parse --show-toplevel)
slug=${branch//\//-}
dest="$root/.worktrees/$slug"

if [[ -e "$dest" ]]; then
  echo "worktree already exists: $dest" >&2
  exit 1
fi

mkdir -p "$root/.worktrees"

if git -C "$root" show-ref --verify --quiet "refs/heads/$branch"; then
  git -C "$root" worktree add "$dest" "$branch"
else
  git -C "$root" worktree add -b "$branch" "$dest" main
fi

echo "$dest"
