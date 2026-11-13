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
      # webkit runs resume-viewer.spec.ts and theme.spec.ts; install it too.
      # WebKit needs GTK/GStreamer/Cairo/Pango system libs on Ubuntu.
      sudo /usr/bin/apt-get update -qq
      sudo /usr/bin/apt-get install -y -qq \
        libgstreamer1.0-0 libgmodule-2.0-0 libgtk-4-1 libpangocairo-1.0-0 \
        libpango-1.0-0 libgdk-pixbuf-2.0-0 libcairo-gobject2 libcairo2 \
        libvulkan1 libgraphene-1.0-0 libglib2.0-0 libatomic1 libsystemd0 \
        libssl3 libxml2 libsqlite3-0 libxslt1.1 liblcms2-2 libevent-2.1-7 \
        libopus0
      pnpm exec playwright install chromium webkit
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
