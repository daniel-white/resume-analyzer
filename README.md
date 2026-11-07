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
- Sign-in emails: <https://mail.resume.local>
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

## Checks

```sh
just check
```
