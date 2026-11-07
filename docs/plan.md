# Resume Analyzer — Hackathon Plan

A job seeker uploads a PDF or DOCX and watches a fleet of simulated applicant
tracking systems (ATS) parse and score it the way those systems actually ingest
a file. The score measures parse survival, not fit against a job description.
There is no job-description matching in this pass.

The demo contrast is the product. A designed resume (contact in the header,
skills in a table, icon bullets, two columns) comes back torn up by some of the
simulated systems and mostly intact from others. A plain single-column resume
scores high across the fleet.

## Decisions

- One Rust binary. A Cargo workspace gives crate boundaries; one binary is
  deployed.
- An in-process event bus, with Postgres as the source of truth through a
  transactional outbox. A restart or a second replica does not lose or
  double-run work.
- ATS integrations are simulated and compiled in. Real vendor APIs need OAuth
  applications and partner access that a one-day hackathon cannot get, so the
  fleet is four fictional systems modeled on publicly described parser
  behavior. No trademarked names or logos.
- Plugins do not return a verdict into the worker. They POST a signed callback,
  the same path a real integration would use later.
- DOCX and PDF are both first-class. The app reads the file once, up front, so
  the user can check that the name, contact, and sections are really there.
  That verification parse is not what the vendors score. Each plugin then reads
  the stored bytes itself and emits its own `ParseInput`. The shape is shared;
  the reads are not. A DOCX read is `structural` because the markup names
  tables, text boxes, and headers. A PDF read is `inferred` because those
  signals are guessed from positioned text.
- A resume belongs to one user and keeps every upload as a version. The object
  key is `users/{user_id}/resumes/{resume_id}/versions/{version_id}`. Deleting a
  user removes that user's rows and every object under the prefix. A later
  version does not erase the earlier file or its results.
- The frontend is a TanStack Start application, its own container. It never
  owns data. Its server functions fetch the Rust API and forward the session
  cookie.
- A Nix flake pins the Rust and Node toolchains. Container images use plain
  Dockerfiles. Building images from Nix closures is deferred.
- Authentication is a magic link through Mailpit first. Passkeys are a
  follow-up branch, and only if time remains, because they need a secure
  context and work on localhost only.
- S3 access goes through the AWS SDK pointed at a custom endpoint, so
  LocalStack in development and a real bucket later are the same code.

## Repository layout

```text
crates/
  app/         binary: HTTP server, workers, server-sent events, wiring
  domain/      resume, runs, events, the ATS plugin trait
  plugins/     the four simulated ATS integrations
  storage/     database access, object storage, outbox
  auth/        magic link, passkeys, sessions
  mail/        SMTP delivery
migrations/
web/           TanStack Start application
  src/routes/      file routes
  src/fns/         server function wrappers
  src/server/      server-only fetch helpers
  src/components/
deploy/helm/resume-analyzer/   chart Minikube installs
docker-bake.hcl                 API and web image targets
scripts/                        worktrees, the check gate, bake, cluster
docs/                           plan.md, contracts.md, factory.md
factory/waves.tsv               catalog of waves the factory acts on
.opencode/skills/ats-plugin/    procedure for adding a simulated ATS
flake.nix
justfile
```

The Rust stack is axum, tokio, sqlx, the AWS S3 SDK, lettre, and webauthn-rs,
on Rust edition 2024. The web stack is TanStack Start on Vite, with Nitro
producing a Node server (`node .output/server/index.mjs`) and Tailwind v4.

## Event spine

```text
POST /api/resumes
  store the bytes in object storage
  insert the resume, its first version, and an outbox entry for ResumeUploaded
  respond 202

POST /api/resumes/:id/versions
  store another file as the next version of that resume
  the previous version, its parse, and its runs stay where they are

Outbox dispatcher (claims rows with SKIP LOCKED, so replicas are safe)
  ResumeUploaded
    read that version's file once into a verification parse
    store it on the version so the user can check the structure
    write ResumeParsed
  ResumeParsed
    insert one run row and an AtsRunRequested entry per plugin
    mark that version running
  AtsRunRequested
    load that version's bytes from object storage
    the plugin reads the file itself, ignoring the verification parse,
    and builds its own parse and verdict
    it POSTs /api/callbacks/{plugin_id} with an HMAC signature,
    a timestamp, and an idempotency key
    a small delay (300-1200ms) so the cards arrive separately in the UI

POST /api/callbacks/:plugin_id
  verify the HMAC in constant time, reject timestamps skewed past five minutes,
  and ignore a replayed idempotency key
  store that plugin's parse and verdict on its run
  write AtsCallbackReceived

  AtsCallbackReceived
    when every run for that version is terminal, mark the version complete
    push to the server-sent-events hub for that resume
```

The event stream is a live hint only. On reconnect the page reloads state from
Postgres and then subscribes. The HMAC secret is per plugin.

## The simulated fleet

Each integration returns what it kept, what it dropped, a score from 0 to 100,
and a factor list such as "contact in header: -25" or "standard headings: +15".
Each factor carries one sentence of advice. The verdict summary is those
sentences filled into a template: the score, then what to change, then what
helped. There is no second model call per plugin.

| Plugin | Bias | What the demo shows |
| --- | --- | --- |
| Greenline | Standard headings, plain text. Penalizes tables, text boxes, and columns. Ignores the header and footer. | A conventional parser |
| Levr | Section-agnostic, matches skills only as exact tokens, likes bullet density. | Freeform text survives, the skills list does not normalize |
| Workdaye | Drops tables, headers, text boxes, and multi-column layout. Contact must be in the body. Dates must be MM/YYYY. | The designed resume falls apart |
| Ash | Keeps more structure, normalizes skills, tolerates two columns when the reading order is sane. Still drops images. | A newer parser |

Fictional names only, with a subtitle in the interface stating they are
simulated and modeled on publicly described parser behavior.

## Parsing

The app reads the file once before any vendor runs. That parse is stored on
the version and shown as "what we read": name, contact, sections, and the layout
signals, so the user can see that the file yielded a real structure. A second
upload is a new version of the same resume, and the detail page can put the two
results next to each other. Vendors
are not blocked on an approval click. An empty extract is still shown, and the
fleet still runs, because a blank read is itself something to verify.

Each plugin then opens the stored PDF or DOCX and produces its own
`ParseInput`. Two plugins can disagree about an email in the header because
they read the file differently, not because they scored the verification parse
two ways.

A DOCX reader unzips the package and walks `word/document.xml` plus the headers.
Tables, text boxes, and header contact come from the markup, so the confidence
is `structural`.

A PDF reader recovers the same signals from positioned text. Column count comes
from clustering on the x axis, header contact from which band the address sits
in, and table detection from aligned blocks. Those signals are `inferred`.

The spine's stub reader only proves the path: it opens the zip or scans the PDF
bytes, then posts a callback. The fleet replaces that reader. A plugin may later
call an OpenAI-compatible model on text it extracted itself. That call belongs
to the plugin. The spine does not make one shared model call for the fleet.

## Frontend

```text
browser
  -> TanStack Start (Nitro, port 3000)   rendering, server functions, session cookie
       -> Rust (axum, port 8080)         the API, the event stream, callbacks, workers
            -> Postgres, object storage, SMTP
```

In development, Vite proxies `/api` to the Rust server with `changeOrigin`
left off, so the cookie stays first-party and the event stream passes through.
In production the chart puts both services behind one ingress, `/` to the web
service and `/api` to Rust, on the same host. One origin means no cross-origin
handling and a `SameSite=Lax` cookie works.

Server functions read `API_URL` and forward the cookie. The browser only ever
talks to its own origin. The event stream is opened directly in the browser
with `EventSource`; it is not tunneled through a server function.

Routes: `/` for upload, `/login`, `/resumes`, and `/resumes/$id`. The detail
page loads state from the API, then opens the event stream.

The interface is a teardown, not a dashboard. One serif face for the score
numerals, monospace for the factor list so it reads like a parser report, and a
color ramp where the score itself is the color. The dropzone comes first. The
detail page shows the verification parse above four cards that fill in as
callbacks arrive. Empty, uploading, and reading states are designed up front.
No stat-card grid and no sidebar. Components come
from shadcn/ui, and the official shadcn skill is installed into the repo so the
agent building the interface reads the project's real configuration instead of
guessing.

## Authentication

One session cookie either way: `HttpOnly`, `SameSite=Lax`, HMAC-signed, backed
by a server-side session row.

Magic link first. The request stores only the SHA-256 of a 32-byte token, valid
for 15 minutes, single use, and provisions the user on first use. lettre sends
the message to Mailpit, whose web interface is part of the demo.

Passkeys second, and only as a follow-up. webauthn-rs, a discoverable
credential, registered after the first login. This needs a secure context, so
it is reached through `kubectl port-forward` to localhost. TLS setup must not
block the demo.

## API surface

```text
POST /api/auth/magic-link
POST /api/auth/magic-link/verify
POST /api/auth/passkey/register/start
POST /api/auth/passkey/register/finish
POST /api/auth/passkey/login/start
POST /api/auth/passkey/login/finish
POST /api/auth/logout
GET  /api/me

POST /api/resumes              multipart, 5MB, PDF or DOCX by magic bytes, 202
POST /api/resumes/:id/versions another file on the same resume, 202
GET  /api/resumes
GET  /api/resumes/:id
GET  /api/resumes/:id/events   server-sent events
POST /api/resumes/:id/rerun

GET  /api/plugins              the quirk legend for the interface
POST /api/callbacks/:plugin_id
```

## Factory

No change lands on `main` directly. Each change is a branch checked out in
`.worktrees/<branch>`, covering one concern, rebased onto current `main`, and
merged only when `scripts/check.sh` exits 0. Agents do not share a worktree, and
only the orchestrating session merges.

The actor is `scripts/factory.sh`, described in `docs/factory.md`. It opens the
worktree, runs the wave, feeds a red gate back into the same worktree, and is
the only thing that merges. A new simulated ATS is `factory.sh act plugin <id>`,
which follows `.opencode/skills/ats-plugin/SKILL.md`. That skill is a procedure,
not an agent definition. No agent profile is committed.

`scripts/check.sh` is the single definition of green:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm typecheck and pnpm test, in web/
scripts/factory_test.sh, when the factory, its catalog, or the ATS plugin skill changed
```

A worktree that touches neither `crates/` nor `web/` skips the half it does not
own, decided from `git diff --name-only main`. A documentation change does not
pay for a Rust build.

Tests are written with the change they cover:

| Where | What it proves | Needs |
| --- | --- | --- |
| `crates/domain` | event names, score math, factor aggregation | nothing |
| `crates/plugins` | each plugin's verdict on the two fixture files, asserted on the factor list rather than the headline number | fixture files |
| `crates/storage` | outbox claims with `SKIP LOCKED` under two concurrent workers, idempotency dedupe, deleting one user leaves the other | a Postgres container the test starts |
| `crates/app` | upload through parse through callback to a terminal state, driven through the router in-process | Postgres |
| `crates/auth` | magic link single use and expiry, session cookie round trip | Postgres |
| `web` | server functions against a mocked API, and a render test that the detail page shows four cards | vitest |

There is no coverage-percentage gate. The assertions above are the gate,
because a percentage would not catch the regression that matters, which is the
fleet agreeing when it should disagree.

### Agent prompts

Every agent prompt carries the same skeleton:

1. The branch name and worktree path. The orchestrator creates the worktree, so
   two agents cannot race `git worktree add`.
2. The files the agent may change. Anything outside that list is a reason to
   stop and report rather than improvise.
3. The path to the contracts document and the specific types the work depends
   on.
4. The definition of done: `scripts/check.sh` green, plus the one behavior the
   wave promised.
5. A size cap. One branch should be reviewable in one sitting. If the work
   needs more, the agent stops and proposes a split.

Agents run as subagents with fresh context. The prompt carries everything they
need, so no custom agent definition is committed. Each agent is instructed to
stay inside its file list and to follow the contracts; those boundaries are
about merge conflicts, not about bypassing anyone's safeguards.

### Waves

| Order | Branch | Owner | Starts when | Done when |
| --- | --- | --- | --- | --- |
| 0 | `factory/bootstrap` | the orchestrator, directly | immediately | the skeleton, the gate, and the contracts headings are committed |
| 1a | `factory/foundation` | foundation agent | wave 0 | `nix develop` builds, `just dev` brings up Postgres, LocalStack, Mailpit, and a healthy API |
| 1b | `web/scaffold` | web-scaffold agent | wave 0 | `pnpm dev` renders the health JSON from Rust |
| 2 | `api/spine` | api agent | 1a | uploading a DOCX and a PDF shows one stub plugin's callback arriving on the event stream |
| 3a | `plugins/fleet` | plugins agent | 2 | the four plugins disagree on the designed fixture and agree on the plain one |
| 3b | `web/ui` | web-ui agent | 1b and the frozen response shapes from 2 | the designed-versus-plain contrast is visible without reading code |
| 4 | `auth/magic-link` | auth agent | 2 and 1b | a magic link round-trips through Mailpit |
| 5 | `demo/minikube` | demo agent | 3a, 3b, 4 | `just demo` on Minikube, plus a three-minute script |

Wave 2 is a sequence of commits on one branch, each leaving the gate green, not
one large commit. The orchestrator merges it once. `auth/passkeys` is
intentionally absent; it is a follow-up only if wave 4 finishes early.

Where berth is available later, `berthd task new` replaces the worktree script
and `berthd exec <worktree> -- scripts/check.sh` is the gate. That config is a
later addition. Wave 0 does not depend on it, and this session has no box.

## Contracts

Written once, in `docs/contracts.md`, before wave 2, and not renegotiated
mid-wave:

- `ParseInput`: one shape, two writers. The resume's parse is the app's
  verification extract. A run's parse is what that plugin read from the bytes.
- Event names: `ResumeUploaded`, `ResumeParsed`, `AtsRunRequested`,
  `AtsCallbackReceived`.
- Callback body: run id, that plugin's parse, and the verdict. HMAC over
  timestamp plus body, five-minute skew, idempotency key. The callback does
  not carry the verification parse.
- Ownership: every resume has a user id and one or more versions. Object keys
  are `users/{user_id}/resumes/{resume_id}/versions/{version_id}`. This spine
  uses a demo user until the magic-link wave supplies the session, and account
  removal deletes the rows and the objects.
- Response shapes for `POST /api/resumes`, `POST /api/resumes/:id/versions`,
  `GET /api/resumes/:id`, and the event stream. Detail returns versions newest
  first, each with its own parse and runs.
- Plugin verdict: kept, dropped, score, a summary, and a factor list. Each
  factor has one sentence of advice. The summary is a template of that advice.

## Development modes

| Command | What runs where | Use |
| --- | --- | --- |
| `just dev` | A `resume-analyzer` Minikube profile runs Postgres, LocalStack, and Mailpit. Envoy Gateway terminates mkcert TLS for `https://app.resume.local` and `https://mail.resume.local`, announced over mDNS at the node IP. Port-forwards let `cargo run` on the host reach the dependencies | building |
| `just demo` | `docker buildx bake`, `minikube image load`, Helm upgrade with the API and web pods enabled, same hostnames | what judges see |

The Nix flake provides two dev shells, `default` for Rust and `web` for Node,
because a single shell carrying both toolchains is where native dependencies
break. `direnv` with `use flake` is the expected entry.

## Follow-ups

Not part of the current wave. Do these after the demo path works, not instead of it.

- [ ] Copy engops' bake versioning. Split `docker-bake.hcl` into a root file
      plus `images/rust` and `images/nodejs`. Shared variables (`REGISTRY`,
      `TAG`) and a `_common` target live in the root. `RUST_VERSION` and
      `NODEJS_VERSION` are declared with no default, so an unset value fails
      the build instead of pulling a floating tag. The flake devShell exports
      them from the pinned toolchain (`rustc --version`, `node --version`).
      Dockerfiles take those as build args. Image tags are
      `${REGISTRY}/<name>:${TAG}`. Until this lands, the Dockerfiles pin
      `rust:1.99.1` and `node:22` by hand, matching the flake.

## Out of scope

Real vendor OAuth, job-description fit scores, dynamically loaded plugins, a
separate message broker, the frontend owning a database, Nix-built container
images, multi-tenant organizations, coverage-percentage gates, and a shared
worktree for drive-by fixes.
