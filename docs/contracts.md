# Contracts

Frozen by the API spine. Plugin and UI waves consume these shapes and do not
invent a second one beside them. JSON is snake_case. Timestamps are RFC 3339.
Identifiers are UUID strings.

Auth routes are not part of this spine. Until a session exists, uploads belong
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
public delete route in this spine.

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
`AtsCallbackReceived`.

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
`verdict` is a Verdict or null. Runs belong to one version.

```text
GET /api/resumes/:id/events
```

`text/event-stream`. Each message's event name is the event `name`, the SSE
id is the event id, and `data` is the event object. The stream is a live hint
only: it does not replay. The page loads `GET /api/resumes/:id` and then
subscribes. A resume owned by someone else is `404`.

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

This spine compiles in one plugin, `stub`. The four simulated systems replace
it without changing this document. Each of them receives the raw file.

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
