# Compiled local demo

From the worktree, run `nix develop` followed by `just demo`.
This bakes and loads both the Rust API and the compiled TanStack Start/Nitro
web app. It never relies on a previously built web image. The existing
`resume-analyzer` Minikube profile and its dependency data are reused.

The gateway serves one HTTPS origin:

- `https://app.resume.local/`: compiled web app, port 3000.
- `https://app.resume.local/api`: Rust API, port 8080.
- `https://tools.resume.local/`: dev tools landing page.
- `https://tools.resume.local/mail`: Mailpit, for sign-in links and analysis emails.
- `https://tools.resume.local/pgadmin`: pgAdmin (when `pgadmin.enabled=true`).

Web server functions use `API_URL=http://api:8080` inside the cluster;
session cookies stay on the browser's own origin. Re-running `just demo`
replaces the API and web pods even though their local `:dev` tags are reused.
Images are streamed from Docker as fresh archives rather than loaded by tag,
so Minikube cannot reuse an older cached `:dev` image. An `account returned 404`
error can indicate a stale API image; re-run `just demo` to replace it.

## Three-minute walkthrough

1. Open the app and request a sign-in link with a demo email.
2. Open Mailpit, follow that link, and confirm the library renders.
3. Upload a designed DOCX with header contact and tables. Wait for the
   parser cards and compare their kept/dropped fields and factors. Fake providers
   finish at staggered times over 10–30 seconds (`ATS_DELAY=min,max` overrides
   these defaults; use `ATS_DELAY=0,0` for fast automated checks).
   Results arriving within an eight-second window are batched into one email
   per resume version. Check Mailpit and follow a provider's link: it selects
   that exact version and opens its result card. Live cards update immediately;
   email batching does not slow them down.
4. Upload a plain DOCX or PDF and compare its results with the designed file.
5. Sign out and confirm the library sends you back to sign-in.
   Open an analysis email while signed out, request a sign-in link, and confirm
   signing in returns to the same resume version/provider rather than the library.

Analysis emails go to the account owner, not an address extracted from the
resume. Delivery uses the existing `SMTP_HOST`/`SMTP_PORT` and `APP_ORIGIN`.
Pending batches persist in Postgres and retry on SMTP failure or restart.
Callback replays and previously completed results do not trigger new emails.
As with ordinary SMTP delivery, a crash after SMTP acceptance but before recording
success can cause a duplicate delivery.

`/health` is the compiled web app's dependency report. It should say
`ok: postgres ok, s3 ok, smtp ok`. If mDNS does not resolve the hostnames,
check `minikube ip --profile resume-analyzer` and the driver's L2 access.
The ATS services remain private ClusterIP services, not gateway routes.

## Checks

`scripts/check.sh` includes `scripts/demo_test.sh` for deployment changes.
That offline check verifies the bake targets, the compiled web deployment,
and the `/` versus `/api` routes. Build the web image and visit `/health`
through the gateway to verify the compiled server against live dependencies.

If Docker reports `unsupported argv0 "docker-credential-osxkeychain"`,
fix the local credential helper first; this is a host toolchain problem,
not a web or gateway failure.
