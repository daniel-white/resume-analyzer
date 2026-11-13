#!/usr/bin/env bash
# The gate every wave has to pass. On a branch with `main` available, only the
# halves this branch touches are built. Otherwise every half that exists runs.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

run_rust=0
run_web=0
run_factory=0
run_demo=0

base=""
if git rev-parse --verify --quiet main >/dev/null; then
  base=main
elif git rev-parse --verify --quiet origin/main >/dev/null; then
  base=origin/main
fi

if [[ -n "$base" && "$(git rev-parse HEAD)" != "$(git rev-parse "$base")" ]]; then
  files=$(
    {
      git diff --name-only "$base"...HEAD
      git diff --name-only
      git diff --name-only --cached
      git ls-files --others --exclude-standard
    } | sort -u
  )
  if printf '%s\n' "$files" | grep -Eq '^(crates/|Cargo\.(toml|lock)$)'; then
    run_rust=1
  fi
  if printf '%s\n' "$files" | grep -Eq '^(web/|Dockerfile\.web$|scripts/web-assets\.mjs$)'; then
    run_web=1
  fi
  if printf '%s\n' "$files" | grep -Eq '^(scripts/(factory(_test)?\.sh|check\.sh)|factory/|\.opencode/skills/ats-plugin/)'; then
    run_factory=1
  fi
  if printf '%s\n' "$files" | grep -Eq '^(Dockerfile(\.web|\.model)?$|docker-bake\.hcl$|deploy/|scripts/(up|demo_test)\.sh$)'; then
    run_demo=1
  fi
else
  run_rust=1
  run_web=1
  run_factory=1
  run_demo=1
fi

if [[ ! -f Cargo.toml ]]; then
  run_rust=0
fi
if [[ ! -f web/package.json ]]; then
  run_web=0
fi
if [[ ! -f scripts/factory_test.sh ]]; then
  run_factory=0
fi
if [[ ! -f scripts/demo_test.sh ]]; then
  run_demo=0
fi

if [[ "$run_rust" == 0 && "$run_web" == 0 && "$run_factory" == 0 && "$run_demo" == 0 ]]; then
  echo "check: nothing to build"
  exit 0
fi

if [[ "$run_rust" == 1 ]]; then
  echo "check: rust"
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
fi

if [[ "$run_web" == 1 ]]; then
  echo "check: web"
  (
    cd web
    pnpm install --frozen-lockfile
    pnpm typecheck
    pnpm test
    pnpm build
    if [[ -f ../scripts/web-assets.mjs ]]; then
      node ../scripts/web-assets.mjs .output
    fi
    if [[ -f playwright.config.ts ]]; then
      echo "check: web playwright"
      pnpm exec playwright install chromium
      # Test the production build we just produced, not the dev server.
      PLAYWRIGHT_PRODUCTION=1 pnpm test:e2e
    fi
  )
fi

if [[ "$run_factory" == 1 ]]; then
  echo "check: factory"
  bash scripts/factory_test.sh
fi

if [[ "$run_demo" == 1 ]]; then
  echo "check: demo"
  bash scripts/demo_test.sh
fi
