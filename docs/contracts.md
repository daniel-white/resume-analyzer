# Contracts

Frozen by the API contract. Plugin and UI waves consume these shapes and do not
invent a second one beside them. JSON is snake_case. Timestamps are RFC 3339.
Identifiers are UUID strings.

Auth routes are not part of this contract. Until a session exists, uploads belong
to the demo user `00000000-0000-0000-0000-000000000001`. List, detail, and the
event stream are already scoped to the calling user. Account removal deletes
that user's rows and every object under `users/{user_id}/`.

## Ownership

A resume belongs to one user and has one or more versions. Each upload is a
version, newest first, so a later file can be compared with the one before it.
The object key is
`users/{user_id}/resumes/{resume_id}/versions/{version_id}`. Cleanup deletes
the outbox rows, the runs, the callback keys, the versions, and the resume
rows for that user, then deletes every object the call returned. There is no
public delete route in this contract.

## ParseInput

One shape, two writers. The app produces one and stores it on the version, so
the user can check that the file has a name, contact, and sections. Each plugin
produces another by reading that version's raw file, and stores it on its run.
The verification parse is not the plugin input. Plugins do not share a parse,
and versions do not share one either.

DOCX sets `layout_confidence` to `structural` because the markup names tables,
text boxes, and headers. PDF sets `inferred` because those signals are guessed
from positioned text.

```json
{
  "profile": {
    "name": "Ada Lovelace",
    "email": "ada@example.com",
    "phone": null,
    "sections": [
      {
        "heading": "Experience",
        "body": "Built things.",
        "in_table": false,
        "in_text_box": false,
        "in_header": false
      }
    ]
  },
  "layout": {
    "columns": 1,
    "contact_in_header": true,
    "has_tables": false,
    "has_text_boxes": false,
    "has_images": false,
    "bullet_count": 0,
    "dates": "absent"
  },
  "layout_confidence": "structural"
}
```

`dates` is `mm-yyyy`, `other`, or `absent`. `columns` is an integer greater
than or equal to 1. `name`, `email`, and `phone` are strings or null.

## Events

`name` is exactly one of `ResumeUploaded`, `ResumeParsed`, `AtsRunRequested`,
`AtsCallbackReceived`, `GenerateMarkdown`, `AnalyzeFeedback`,
`GenerationStream`.

```json
{
  "id": "uuid",
  "resume_id": "uuid",
  "version_id": "uuid",
  "name": "AtsRunRequested",
  "payload": {},
  "created_at": "2026-11-07T18:00:00Z"
}
```

Payloads:

| Name | Payload |
| --- | --- |
| `ResumeUploaded` | `{ "user_id", "resume_id", "version_id", "object_key", "filename", "media_type" }` |
| `ResumeParsed` | `{ "parse": ParseInput }` |
| `AtsRunRequested` | `{ "run_id", "plugin_id" }` |
| `AtsCallbackReceived` | `{ "run_id", "plugin_id", "parse": ParseInput, "verdict": Verdict }` |
| `GenerateMarkdown` | `{ "version_id" }` |
| `AnalyzeFeedback` | `{ "version_id", "user_id", "section_id", "section_heading", "section_kind" }` |
| `GenerationStream` | a `GenerationFrame`, described with the SSE stream below |

`media_type` is `application/pdf` or
`application/vnd.openxmlformats-officedocument.wordprocessingml.document`.
`object_key` is `users/{user_id}/resumes/{resume_id}/versions/{version_id}`.

`ResumeUploaded` is the app's read of that version. It stores the verification
parse and emits `ResumeParsed`. `ResumeParsed` fans out one run per plugin for
that version only. `AtsRunRequested` loads the version's object and calls that
plugin with the raw file and the webhook URL for `POST /api/callbacks/:plugin_id`.
The plugin reads the file itself and does not receive the verification parse.
It posts the signed callback to that URL. The worker does not store the
in-process return value. An empty verification parse still fans out. Nothing
waits for the user to approve it. A new version does not rerun the previous one.

`ResumeUploaded` reads the file once, detects sections, and makes one combined
model request for the profile impression: owner-facing prose, a verbatim-quoted
skill per `field` event, and the progress stages. Rust drops any skill whose
quote is not found in the cited stored section, merges aliases, and computes
importance from prominence and evidence breadth. The one returned
`ProfileImpression` is saved as the profile summary and the validated skill set;
there is no second model read and no separate skill outbox event. A skill quoted
solely from a bare list is kept but flagged as needing a concrete example in the
UI. Confidence below 20% is also kept in the stored set, but Rust drops those
skill frames from `GenerationStream` and the page hides them; the threshold is
inclusive, so 20% is presented. An empty, fully grounded result is valid; a
model that invents every quote is an error and neither reading is saved. Live
frames route by content: prose, `is_resume`, and the `reading` and `understanding`
stages go to the `summary` flow, while skill fields and the `mapping` stage go
to the `skills` flow. `queued` and `complete` stages are sent to both flows.

When the registry gains a plugin, an authenticated detail visit reconciles the
newest parsed version with the active registry. Missing runs and their
`AtsRunRequested` events are created atomically and only once. Existing results
and older versions are preserved. A completed version returns to `running`
until the newly requested callbacks arrive; a missing run is not proof that a
provider is already reading the file.

## Callback

```text
POST /api/callbacks/:plugin_id
content-type: application/json
x-timestamp: unix seconds
x-signature: lowercase hex HMAC-SHA256
idempotency-key: opaque string, unique per plugin
```

The signature covers the ASCII bytes of `{timestamp}.` concatenated with the
raw request body, keyed with that plugin's secret. Comparison is constant
time. A timestamp skewed by more than 300 seconds is rejected. The signed
bytes are the body as received, not a re-serialized document.

```json
{ "run_id": "uuid", "parse": {}, "verdict": {} }
```

`parse` is the ParseInput that plugin produced from the raw file. The worker
does not store the in-process return value. Only this callback is stored.

| Status | Meaning |
| --- | --- |
| 200 | Stored (`{ "status": "recorded" }`), or a replay of the same idempotency key (`{ "status": "replay" }`). A replay writes nothing. |
| 401 | Signature or skew failed. |
| 404 | Unknown plugin, or the run does not belong to it. |

### Analysis notifications

A newly recorded callback also marks its run for email delivery in the same
transaction. After an eight-second batching window, a separate worker sends
all pending completed runs for that resume version to the account owner's email.
It does not wait for still-running providers and does not block live events.
SMTP failures remain pending; successful runs are not emailed again on callback
replay. Existing results are not backfilled when this feature is deployed.

Each result links to `/resumes/{resume_id}?version={version_id}&provider={plugin_id}`
on `APP_ORIGIN`. The page selects that version and expands the provider card.
Signed-out visitors retain this destination through magic-link login.
`POST /api/auth/magic-link` accepts optional `return_to` alongside `email`;
only local resume-detail paths are accepted, never external redirect URLs.

## HTTP

```text
POST /api/resumes
```

Multipart field `file`. The limit is 5242880 bytes. The type comes from magic
bytes: a PDF starts with `%PDF`, a DOCX is a zip that contains
`word/document.xml`. Anything else is rejected. The client content type is
ignored.

`202`

```json
{ "id": "uuid", "version_id": "uuid", "status": "uploaded" }
```

`id` is the resume. `version_id` is this upload. A later file on the same
resume is `POST /api/resumes/:id/versions` and returns the same shape with the
existing resume id.

`413` over the limit, `415` for a file that is neither PDF nor DOCX, `400`
when the field is missing.

```text
GET /api/resumes
```

The calling user's resumes, newest first.

```json
{
  "resumes": [
    {
      "id": "uuid",
      "title": "ada.pdf",
      "created_at": "...",
      "latest": {
        "id": "uuid",
        "number": 2,
        "filename": "ada.pdf",
        "status": "uploaded",
        "created_at": "..."
      }
    }
  ]
}
```

`title` starts as the first filename. `latest` is the highest version number.

```text
GET /api/resumes/:id
```

`404` when the id is unknown or belongs to someone else.

```json
{
  "id": "uuid",
  "title": "ada.pdf",
  "created_at": "...",
  "versions": [
    {
      "id": "uuid",
      "number": 2,
      "filename": "plain.pdf",
      "media_type": "application/pdf",
      "status": "running",
      "created_at": "...",
      "parse": null,
      "sections": null,
      "skills": null,
      "runs": [
        {
          "id": "uuid",
          "plugin_id": "stub",
          "status": "requested",
          "parse": null,
          "verdict": null
        }
      ]
    }
  ]
}
```

`versions` is newest first. A version's `status` is `uploaded`, `parsed`,
`running`, or `complete`. `parsed` means that version's verification extract is
stored and no vendor has been requested yet. A run's `status` is `requested` or
`complete`. The version's `parse` is the verification ParseInput, or null while
the file is still unread. A run's `parse` is what that plugin read, or null.
`verdict` is a Verdict or null. Runs belong to one version. `sections` is the
app's `SectionMap` (detected headings, kinds, and bodies), or null while unread.
`skills` is the validated `SkillSet`, or null until the upload's combined
impression finishes:

```json
{
  "engine": "ollama",
  "model": "llama-3.2-3b-instruct-q4_k_m",
  "skills": [{
    "name": "Triage",
    "canonical": "triage",
    "category": "method",
    "nature": "hard",
    "importance": 86,
    "confidence": 91,
    "confidence_reason": "Grounded in a concrete accomplishment.",
    "evidence": [{ "section_id": "uuid", "quote": "Triaged patients in the emergency department." }]
  }]
}
```

`category` is job-agnostic: `tool`, `method`, `craft`, `practice`, `domain`,
`creative`, or `other`. `nature` is the contextual second axis: `technical`,
`hard`, `people`, `soft`, or `domain`. `importance` and `confidence` are 0–100;
`importance` is computed in Rust, not by the model or the UI. `confidence_reason`
is the model's short explanation of its confidence and is stored for review.
Each `evidence` quote is copied verbatim from the cited `sections` entry. The
comparison view sends a compact summary (`name`, `canonical`, `nature`,
`importance`) of presented skills per version and derives gained, lost, and
shifted tags from it.

```text
GET /api/resumes/:id/events
```

`text/event-stream`. Each message's event name is the event `name`, the SSE
id is the event id, and `data` is the event object. The stream is a live hint
only: it does not replay. The page loads `GET /api/resumes/:id` and then
subscribes. A resume owned by someone else is `404`.

Summary, feedback, and skill generations also emit `GenerationStream` events on
this channel. Their `payload` is a `GenerationFrame`:

```json
{
  "id": "e7f1c2a4-...",
  "resume_id": "9c8b7a6d-...",
  "version_id": "1f2e3d4c-...",
  "name": "GenerationStream",
  "payload": {
    "resume_id": "9c8b7a6d-...",
    "version_id": "1f2e3d4c-...",
    "flow": "feedback",
    "section_id": "5a6b7c8d-...",
    "seq": 4,
    "event": { "type": "delta", "field": "text", "value": "The heading " }
  },
  "created_at": "2026-11-17T14:08:38Z"
}
```

`flow` is `summary`, `feedback`, or `skills`; `section_id` is null for summary
and skill runs and set for a voted-down section diagnosis. `seq` increases per run and lets the
client discard duplicates or late frames. `event` is one NDJSON line parsed
from the model: `stage`, `delta`, `field`, `done`, or an app-generated `error`.
Summary deltas use field `profile_text` and `field` events set `is_resume`;
feedback deltas use field `text`. String-valued `field` events for those text
fields are also concatenated as prose, because small models often emit a whole
paragraph this way instead of many `delta` lines. The worker emits `queued` first and
`complete` only after the final text has been written to Postgres. These
frames are hints; authoritative results remain in the resume detail response.

The skill flow emits one `field` event named `skill` per tag, with an object as
`value` instead of a string:

```json
{
  "type": "field",
  "name": "skill",
  "value": {
    "name": "Triage",
    "canonical": "triage",
    "category": "method",
    "nature": "hard",
    "prominence": 5,
    "confidence": 91,
    "confidence_reason": "Grounded in a concrete accomplishment.",
    "evidence": [{ "section_id": "uuid", "quote": "Triaged patients..." }]
  }
}
```

The client treats these as provisional: each frame pops a new tag into the
cloud while quote validation and importance scoring finish on the server. The
worker drops skill frames under 20% confidence before they reach the event
stream, though those skills remain in the stored set. A
tag is only authoritative once `versions.skills` appears on the next detail
read. `prominence` (1–5) sizes live tags; the stored `importance` (0–100)
replaces it. `nature` may be omitted by a small model, in which case the client
and server both derive it from `category`.

Diagnosis text is admin-only. Admins subscribe to the separate
`GET /api/admin/events` stream, which is session-cookie protected and forwards
only `GenerationStream` feedback frames. Owner streams never include diagnosis
deltas.

```text
GET /api/plugins
```

```json
{
  "plugins": [
    { "id": "stub", "name": "Stub", "subtitle": "Stand-in parser. The simulated fleet replaces this." }
  ]
}
```

This build compiles in one plugin, `stub`. The four simulated systems replace
it without changing this document. Each of them receives the raw file.

```text
GET /api/version
```

Build metadata for the About page. No session or database is required.

```json
{ "version": "0.2.4", "commit": "9f3c1a2", "dev": true }
```

`version` is the workspace semver from `Cargo.toml`. `commit` is the first seven
characters of the git commit, or `unknown` when the build has no git metadata.
`dev` is true for local builds and false for release images.

## Verdict

```json
{
  "kept": ["email"],
  "dropped": ["header contact"],
  "score": 35,
  "summary": "Score 35 of 100. Move the email and phone into the body, not the header.",
  "factors": [
    {
      "label": "contact in header",
      "delta": -25,
      "advice": "Move the email and phone into the body, not the header."
    }
  ]
}
```

`score` is an integer from 0 to 100. `delta` is a signed integer. `advice` is
one sentence: a negative factor says what to change, a positive factor says
what to keep. `summary` is the score, then the negative advice, then the
positive advice. It is that template, not a model call. The score is
`clamp(base + sum(delta), 0, 100)`. Each plugin chooses its own base. The stub
uses 50.

### Simulated provider report (optional)

Python providers attach `verdict.report` to the signed callback. Rust stores
and returns it unchanged with the verdict, including in callback events. Old
results and the compiled stand-in omit this field; the UI must not fabricate it.

```json
{
  "engine": "Plaintext Classic",
  "engine_version": "3.8.2",
  "reference": "GREENLINE-38F60C123ABC",
  "outcome": "parsed_with_warnings",
  "contact_fields": 2,
  "sections_detected": 1,
  "bullets_detected": 3,
  "layout_confidence": "structural",
  "notes": ["Layout signals detected from DOCX markup."]
}
```

Engine identity/version and report references are fictional, not vendor claims.
The reference is deterministic per provider/run, so retries keep identical body
bytes. `outcome` is `parsed`, `parsed_with_warnings` (negative score factors), or
`unreadable`. Counts describe the provider's raw-file detection, not field
survival or verified candidate data. Unreadable reports show no extraction
metrics in the UI. PDF notes disclose the limited embedded-text inspection and
inferred layout evidence. Reports do not change score math or factor templates.

The cross-runtime signature/serialization check is opt-in because the Rust-only
toolchain does not include Python:

```sh
PYTHON=python3 cargo test -p domain --test python_report -- --ignored
python3 -m unittest discover -s providers -p 'test_*.py'
```
