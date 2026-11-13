#!/usr/bin/env bash
# Set RUST_VERSION, NODEJS_VERSION, and PNPM_VERSION from the flake devShell.
#
# The flake is the only source of these versions. Always ask `nix develop`,
# even when the caller is already inside a shell: a long-lived shell can be
# several flake.lock generations behind, and its PATH would silently build the
# images with an older compiler or Node.
#
# Do not read rust-toolchain.toml, .node-version, or package.json here. Those
# duplicates drift from flake.lock and produce images built with a different
# toolchain than the one CI verifies.

if ! command -v nix >/dev/null 2>&1; then
  echo "versions.sh: nix is required; run inside 'nix develop' or install Nix" >&2
  exit 1
fi

root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
# The shellHook prints a greeting before the command runs. Keep only the last
# line so that greeting never becomes part of the parsed values.
versions=$(
  nix develop "$root" --command bash -c '
    printf "%s %s %s\n" \
      "$(rustc --version | cut -d" " -f2)" \
      "$(node --version | cut -c2-)" \
      "$(pnpm --version)"
  ' 2>/dev/null | tail -n 1
)
read -r RUST_VERSION NODEJS_VERSION PNPM_VERSION <<<"$versions"

if [[ -z "$RUST_VERSION" || -z "$NODEJS_VERSION" || -z "$PNPM_VERSION" ]]; then
  echo "versions.sh: could not resolve tool versions from the Nix devShell" >&2
  exit 1
fi

export RUST_VERSION NODEJS_VERSION PNPM_VERSION
