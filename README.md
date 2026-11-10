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

For local LLM summaries, run a llama.cpp server, which speaks the OpenAI chat
completions API, and point the API at it:

```sh
llama-server -m llama3.2-3b-q4_k_m.gguf --host 127.0.0.1 --port 8080
```

The API warms `llama3.2-3b-q4_k_m` at `http://127.0.0.1:8080/v1/chat/completions`
by default. Set `SUMMARY_MODEL_NAME` or `SUMMARY_MODEL_URL` to change that.
If the model cannot load or generate, the deterministic extractive fallback
is used and identified in the UI. Resume text stays on the configured chat
server; it is sent as JSON user data separate from fixed system instructions,
with no tools available to the model. Output is bounded and rendered as plain
text.

### llama.cpp in the cluster

`just demo` deploys the official `ggml-org/llama.cpp` image as the `llama-cpp`
pod. An initContainer downloads the GGUF into a PVC before the server starts,
so a fresh pod serves it without a download while a resume uploads. A
quantized 3B model answers in well under a second on one core. The API reaches
it at `llama-cpp:8080` on the OpenAI chat completions path. The worker still
parses and stores a `profile_summary` row either way. Point the API at a
different server with `SUMMARY_MODEL_URL`.

## Checks

```sh
just check
```
