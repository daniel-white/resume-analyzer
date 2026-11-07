#!/usr/bin/env bash
# Build images with docker buildx bake. Pass a target or group name
# (`api`, `web`, `all`); the default group is the API only, because the
# web image needs web/ from the scaffold branch.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

exec docker buildx bake --file docker-bake.hcl "$@"
