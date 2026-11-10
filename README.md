# Resume Analyzer

Compare resume parsing results across simulated ATS providers. Rust API,
React/TanStack Start frontend, and local Postgres, S3 (LocalStack), and Mailpit.

## Quick start

Install [Nix](https://nixos.org/download/) with flakes enabled and have Docker
running (with Buildx available). The Nix shell supplies the other tools.
The local cluster defaults to Minikube's `vfkit` driver on macOS; other platforms
can set `MINIKUBE_DRIVER` to a supported driver. Demo hostnames require mDNS
and a cluster IP reachable from your host.

From the repository root:

```sh
nix develop
just demo
```

This creates/reuses the `resume-analyzer` Minikube profile, sets up local TLS
(the first run may ask permission to trust the mkcert CA), and builds and deploys
the API, web app, simulated ATS providers, and dependencies.

- App: <https://app.resume.local>
- Dev tools (Mailpit, pgAdmin): <https://tools.resume.local>
- Sign-in emails: <https://tools.resume.local/mail>
- Dependency health: <https://app.resume.local/health>

Request a sign-in link in the app, open it from Mailpit, then upload a DOCX or PDF.
Re-run `just demo` to rebuild and deploy changes; existing dependency data is reused.
See [the demo guide](docs/demo.md) for a walkthrough and troubleshooting.

## Local development

Inside `nix develop`, start the dependencies and host API:

```sh
APP_ORIGIN=http://localhost:3000 just dev
```

In a second terminal, start the frontend:

```sh
nix develop
cd web
pnpm install
pnpm dev
```

Open <http://localhost:3000>. The frontend proxies `/api` to the Rust API on
port 8080. Read sign-in emails at <http://localhost:8025>.

### Candidate profile model

After parsing, the worker creates a short candidate profile and stores it with
that resume version. The details page labels it as a model impression and lets
the signed-in user vote it up or down. Model code lives under `crates/models`.

For local LLM summaries, install [Ollama](https://ollama.com/), pull the small
default model, and start its server before the API:

```sh
ollama pull qwen2.5:0.5b
ollama serve
```

The API warms `qwen2.5:0.5b` at `http://127.0.0.1:11434` by default. Set
`SUMMARY_MODEL_NAME` or `SUMMARY_MODEL_URL` to change that, or
`SUMMARY_MODEL=off` to always use the deterministic extractive fallback. If the
model cannot load or generate, the same fallback is used and identified in the
UI. Resume text stays on the configured Ollama server; it is sent as JSON user
data separate from fixed system instructions, with no tools available to the
model. Output is bounded and rendered as plain text.

### Ollama in the cluster

`just demo` bakes `Dockerfile.ollama` — Ollama with `qwen2.5:0.5b` already
pulled — and deploys it as the `ollama` pod. The API reaches it at
`ollama:11434`, so the model loads once at startup and is never downloaded while
a resume uploads. Turn it off with `--set ollama.enabled=false` to fall back to
the extractive summary; the worker still parses and stores a `profile_summary`
row either way. Override the model with `--set ollama.model=<name>` and rebuild
the image, or point the API at a different server with `SUMMARY_MODEL_URL`.

## Checks

```sh
just check
```
