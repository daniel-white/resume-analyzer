#!/usr/bin/env bash
# Opt in to the fast preflight. CI keeps running the full gate.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

chmod +x .githooks/pre-commit
git config core.hooksPath .githooks
echo "installed: .githooks/pre-commit (core.hooksPath=.githooks)"
