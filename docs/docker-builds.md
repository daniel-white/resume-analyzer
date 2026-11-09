# Docker builds and caching

From the repository root:

```sh
./scripts/bake.sh all --progress=plain
# Or, from nix develop:
just bake all --progress=plain
```

The default target is `api`; `web` and `providers` can also be built separately.
The scripts use BuildKit through Docker Buildx. No host Rust or Node installation
is required to build the images.

## Cache boundaries

- **API:** Cargo Chef 0.1.78 on Rust 1.99.1 Bookworm prepares a workspace
  recipe, then compiles dependencies in a separate layer. The planner stage only
  reads Cargo.toml/Cargo.lock, so Rust source changes do not invalidate it. The
  cook layer stays cached; source changes only invalidate the application build
  step, which recompiles just the changed crates and their dependents. Migration
  changes only invalidate the application layer. Both compilation steps use
  `--locked` and the same toolchain, profile, and working directory.
- **Downloads:** BuildKit mounts retain Cargo registry/Git downloads and the
  pnpm store across lockfile changes on the same builder.
- **Web:** pnpm setup precedes manifest copies, dependency installation precedes
  source copies, and only `.output` goes into the runtime image.
- **Context:** `.dockerignore` excludes host build outputs, worktrees, local
  certificates, environment files, and test reports.

The compiled Cargo dependencies deliberately live in the cook **layer**, not a
`target/` cache mount. That layer can be exported to another builder; cache
mounts are local to the builder and are not carried by ordinary layer exports.
Cold builds still compile dependencies; no speedup is claimed for that case.

To share intermediate layers with a registry-backed Buildx builder, configure
cache import/export per target (use separate references for API and web):

```sh
./scripts/bake.sh api \
  --set 'api.cache-from=type=registry,ref=YOUR_REGISTRY/resume-analyzer/api:buildcache' \
  --set 'api.cache-to=type=registry,ref=YOUR_REGISTRY/resume-analyzer/api:buildcache,mode=max'
```

Registry export requires an appropriate Buildx driver/container image store and
registry write access. Treat shared build caches as trusted build inputs.

## Local macOS credential failure

If the build fails before compilation with
`unsupported argv0 "docker-credential-osxkeychain"`, a Nix/Home Manager multicall
binary may be shadowing OrbStack's actual credential helper. `scripts/bake.sh`
probes the keychain helper protocol and prefers a working OrbStack helper for
that process only. It does not modify Docker's credential configuration.
`scripts/up.sh` and `scripts/gateway.sh` also apply this workaround before Helm
fetches OCI gateway charts. Prefer these over invoking
`docker buildx bake` directly when affected by this issue.

## Mr Boxington

Cargo Chef is used here because it gives source-independent Docker dependency
layers. [Mr Boxington](https://mr-boxington.jdx.dev/) (`mbx`) is complementary:
it caches individual compiler outputs and can share work across native
worktrees/CI. It is not enabled by these Dockerfiles. A host macOS cache cannot
be assumed reusable for Linux container builds. Adding mbx inside Docker would
need a persistent Linux cache mount or explicitly configured remote cache,
plus measurements to justify the installation and cache-transfer overhead.
