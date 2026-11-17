# Rules

Strict by default. `./scripts/check.sh` is the gate; a red gate is a blocker,
not a follow-up. CI runs the same script.

## Rust and Cargo

- All workspace crates inherit `[workspace.lints]` from the root `Cargo.toml`.
  Rustc `warnings` and clippy `pedantic` + `cargo` are **deny**.
- `.cargo/config.toml` sets `rustflags = ["-D", "warnings"]` for every target.
  Never silence code with `#[allow(warnings)]`, `#[allow(clippy::all)]`, or by
  adding `-A` flags. Fix the code, or allow the one lint at the narrowest scope
  with a comment explaining why.
- Only four lints are allowed workspace-wide: `missing_errors_doc`,
  `missing_panics_doc` (boilerplate docs), `multiple_crate_versions` (dependency
  tree), and `too_many_lines` (SQL mapping and end-to-end scenarios). Do not add
  more without changing this file.
- Run `cargo fmt`. Hand-formatted Rust is not accepted.

## TypeScript

- `web/tsconfig.json` is strict: `strict` plus `noUncheckedIndexedAccess`,
  `exactOptionalPropertyTypes`, `noImplicitOverride`,
  `noFallthroughCasesInSwitch`, `noImplicitReturns`, `noUnusedLocals`,
  `noUnusedParameters`, `noPropertyAccessFromIndexSignature`,
  `allowUnreachableCode: false`, `allowUnusedLabels: false`, and
  `forceConsistentCasingInFileNames`.
- No `any` or non-null assertions (`!`) in hand-written application code. Tests
  may use `@ts-expect-error` with a reason; generated files are exempt. Narrow
  with guards, validate at boundaries, and keep the rest `unknown`.
- `web/src/routeTree.gen.ts` is generated. Do not hand-edit it.

## Helm

- Every chart under `deploy/helm/*` must pass `helm lint --strict`. Warnings
  are failures. Do not add templates that only render when values are set
  without providing those values in `values.yaml`.

## Verification

- Before handoff: `./scripts/check.sh`. CI runs the same script in two named
  steps: `check.sh strict` gates Rust (`fmt`, build, clippy `-D warnings`),
  TypeScript (`tsc --noEmit`), and Helm (`lint --strict`); `check.sh behavior`
  gates tests, e2e, factory, and demo. Each step only runs the halves the
  branch touches.
- `just hooks` installs `.githooks/pre-commit`: a fast preflight on staged files
  (whitespace, fmt, clippy, typecheck, helm lint). It does not run tests or
  replace CI.
- Never weaken a gate to make a change pass.
