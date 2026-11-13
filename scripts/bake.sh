#!/usr/bin/env bash
# Build images with docker buildx bake. Pass a target or group name
# (`api`, `web`, `all`). The default group is the API only. `just demo`
# bakes `api` and `web` and routes both through app.resume.local.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
# shellcheck source=docker-cred.sh
source "$root/scripts/docker-cred.sh"
# shellcheck source=versions.sh
source "$root/scripts/versions.sh"

# Commit time is stable for a checkout and is what SOURCE_DATE_EPOCH is for.
# Do not inherit the value from the interactive shell.
SOURCE_DATE_EPOCH=$(git log -1 --format=%ct 2>/dev/null || echo 0)
BUILD_DATE=$(
  date -u -r "$SOURCE_DATE_EPOCH" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null ||
    date -u -d "@$SOURCE_DATE_EPOCH" +%Y-%m-%dT%H:%M:%SZ
)
if [[ -z "${GIT_SHA:-}" ]]; then
  GIT_SHA=$(git rev-parse HEAD 2>/dev/null || echo unknown)
fi

export RUST_VERSION NODEJS_VERSION PNPM_VERSION SOURCE_DATE_EPOCH BUILD_DATE GIT_SHA

"$root/scripts/bake-manifests"

exec docker buildx bake \
  --file docker-bake.hcl \
  --file images/rust/docker-bake.hcl \
  --file images/nodejs/docker-bake.hcl \
  "$@"
