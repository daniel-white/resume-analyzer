#!/usr/bin/env bash
# Build images with docker buildx bake. Pass a target or group name
# (`api`, `web`, `all`); the default group is the API only.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
# shellcheck source=docker-cred.sh
source "$root/scripts/docker-cred.sh"

exec docker buildx bake --file docker-bake.hcl "$@"
