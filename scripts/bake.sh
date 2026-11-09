#!/usr/bin/env bash
# Build images with docker buildx bake. Pass a target or group name
# (`api`, `web`, `all`). The default group is the API only. `just demo`
# bakes `api` and `web` and routes both through app.resume.local.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
# shellcheck source=docker-cred.sh
source "$root/scripts/docker-cred.sh"

"$root/scripts/bake-manifests"

exec docker buildx bake --file docker-bake.hcl "$@"
