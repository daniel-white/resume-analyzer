# Upload-first landing page

Signed-out visitors see the landing page at `/`. Signed-in visitors still see their resume library.

## Flow

1. `POST /api/uploads` accepts and content-sniffs a PDF or DOCX, up to 5 MiB. It stores private bytes and a `pending_uploads` record with a one-hour expiry. No resume, user, or analysis job exists yet.
2. The landing page sends `upload_id` and a local resume `return_to` to the magic-link endpoint. The upload capability is a random UUID; it must not be logged or exposed as a public file URL.
3. A verified link atomically consumes the pending upload, creates the owned resume and first version, enqueues `ResumeUploaded`, and creates the session. This works when the email link opens in another browser or device; the file does not depend on browser memory or cookies.
4. The existing analysis worker and result page take over. The email link returns the visitor directly to their resume.

The application checks every minute for expired pending files. It deletes the bytes before removing metadata so failed object deletions can retry. Claimed files are no longer pending and are not expired by this cleanup.

## Deployment

Deploy the API and web changes together. The API runs `0004_pending_uploads.sql` through its existing startup migration mechanism. Keep the object bucket private. Apply deployment-level request rate limits to anonymous uploads and magic-link requests before exposing this publicly; file size limits and expiry do not prevent high-volume abuse.

## Checks

- `cd web && pnpm typecheck && pnpm test && pnpm build`
- `cd web && pnpm exec playwright test`
- `cargo test -p app --test pending_upload` (requires Docker)

The new backend test checks private staging, delayed analysis, verified account ownership, single-use claiming, expiry cleanup, retained claimed bytes, and rejected fake PDFs. Browser tests cover the upload/sign-in handoff, validation, and mobile layout. Browser tests use the existing API stub, not a live mail service.
