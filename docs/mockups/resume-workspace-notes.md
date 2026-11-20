# Résumé workspace redesign — design notes

Status: **mockup only**. No app code, styles, tests, backend code, or schema have been changed.
Visual mockup: `docs/mockups/resume-workspace.html` (self-contained HTML/CSS, open in a browser).

## Why this exists

The version detail page currently stacks four long panels in one column — model
impression (`web/src/components/profile-summary.tsx`), skills/evidence map
(`skill-cloud.tsx`), editing checklist (`second-draft.tsx`) and résumé structure
(`resume-structure.tsx`). Each has a 40px icon, its own gradient card and a 28px
serif heading, and they all stream in together via `LiveAnalysis`. The original
document (`resume-viewer.tsx`) sits below everything in `.desk-split`. Result:
long page, oversized type, and facts mixed with suggestions.

## Decisions (latest)

- **One short, flat tab bar on the version page** — no nested modes, no
  button-style area switch. Tabs in order: Overview, Structure, Skills, ATS
  insights. There is deliberately no Impression tab; the impression lives on
  Overview. “ATS results” was renamed because it sounded like search results.
- **Tabs are full path components, not hashes**: `/resumes/$id` is Overview,
  `/resumes/$id/structure`, `/resumes/$id/skills` and
  `/resumes/$id/ats-insights` are the other tabs, and `/resumes/$id/ideas` is a
  separate route. Each view is directly linkable and reloadable; Ideas is not a
  tab inside the version page.
- **Overall grades and systems were dropped from the top of the page** as
  redundant Overview detail. Grades now appear only with the ATS insights they
  belong to (the ATS tab and one Overview signpost). The only header status is
  “Analysis complete · uploaded 12 Nov”.
- **Overview is the default tab** and holds a small Quick insights strip plus
  the model impression as the lead finding. The insight cards are only
  signposts to other tabs. The impression box is labelled simply “Model
  impression”.
- **No repeated headings or eyebrow labels.** Each pane has one visible title;
  the "In the document" / "Suggestions" eyebrow lines, section labels, and
  “Who the model thinks you are” heading were removed from the mockup.
- **Ideas is a separate page**, not a tab. Overview carries “We found things to
  improve — 14 ideas for this version, 6 already done →”. Structure, Skills and
  ATS insights each carry a smaller, contextual “Want to improve this? See the
  N ideas about … →” call to action. Three placements were explored: a dashed
  inline card at the bottom of a pane (default), a quiet text link after a
  single finding, and a summary strip at the top of a pane. The Ideas page has
  its own progress, group headings, per-item notes, done states, filters and a
  context rail; it never edits the source document. Its header is deliberately
  similar but distinct: the same résumé name, an “Improvement ideas · v3”
  eyebrow instead of “Résumé analysis · v3”, and an “Ideas only · N of M done ·
  your PDF is unchanged” subtitle. The back link returns to the analysis.
- **The current résumé stays visible** in a sticky right-hand rail on the
  version page, with a full preview and a sticky bottom bar on mobile. The
  Ideas page swaps this for an excerpt context card.
- **“Upload new version” is a prominent lime action button beside the version
  selector** on desktop and a short “Upload” button beside the pill on mobile.
  The custom dropdown now holds versions only — no upload and no delete.
- **Deletion moves to a quiet danger area at the bottom of Overview**, scoped
  to the version being viewed. There is one action, “Delete this version”, with
  no ellipsis and no “Delete all versions…”. It opens a focused confirmation
  dialog; cancel returns focus to the button.
- **Rename stays inline** beside the résumé name, applies to all versions, and
  is distinct from the fixed source filename.
- **Mobile states are explicit**: a short horizontally scrollable tab bar, a
  version-picker overlay under the pill, inline rename under the title, an
  Ideas page with its own header and filters, the danger area on Overview, and
  a pinned preview bar on the version page.
- **“Show in résumé” section jumps / in-document highlights are skipped** by
  decision. No deep-link or text-layer highlighting work is in scope.

## Research notes (for later; not in scope)

### Shareable link to the original document (backend)

Uploaded bytes live in a **private S3 bucket** (LocalStack locally), keyed
`users/{user_id}/resumes/{resume_id}/versions/{version_id}`. The frontend URL
`/api/resumes/{id}/versions/{version_id}/file` is built in
`web/src/server/resumes.ts:509-512`; it is a same-origin, session-cookie-only
proxy (`crates/app/src/api.rs:228-260`), not a public or presigned URL. There is
no existing share token, presigned URL, public URL or file expiry for claimed
files.

Options, smallest first:

1. **HMAC capability on the existing proxy route** — owner mints
   `?token=…&expires=…` using existing `AUTH_SECRET` helpers; route verifies the
   token and serves inline with `private, no-store`. No new storage. Bearer link
   until expiry; revocation needs a stored nonce.
2. **DB-backed share table** — `file_shares(token_hash, version_id, expires_at,
   revoked_at)` plus a dedicated route. Adds revocation/audit; must invalidate
   shares on delete.
3. **Presigned S3 GET** — the pinned `s3` crate supports `presign_get`, but the
   configured endpoint is in-cluster (`http://localstack:4566`), not
   browser-reachable; needs a public endpoint/CORS and is hard to revoke.
4. **Public bucket/CDN copy** — risky and explicitly against the private-bucket
   requirement in `docs/upload-first.md`; keys encode user/resume IDs.

Never expose object keys, and never turn the `pending/{uuid}` upload capability
into a file URL.

### Jump to a section / highlight skills inside the PDF preview (skipped)

Clarified requirement: “link in the original document” meant the document
preview itself jumping to a section or highlighting a skill, not a share URL.
Feasibility, recorded in case it is revisited:

- `web/src/components/resume-document.tsx` uses `DocViewer` from
  `@iamjariwala/react-doc-viewer@1.9.0` (react-pdf 9.2.1 / pdfjs-dist 4.8.69).
  Text selection and search are enabled, so pdfjs renders a `.textLayer`, and
  `DocViewerRef.goToPage(page)` is public.
- A frontend-only PDF version is feasible: find the target text in the text
  layer per page, call `goToPage`, scroll within `.rdv-pdf-main-content`, and
  draw a highlight overlay. Use the **verbatim evidence quote** rather than the
  skill name — skill names are often not written literally in the document.
- Limitations: no page/offset/bbox data exists anywhere in the API (PDF
  extraction discards it); headings can be normalized or repeated; image-only
  PDFs have no text layer and there is no OCR; DOCX has DOM text but no pages;
  WebKit PDF scrolling is unreliable in existing e2e tests.
- Robust version (backend): persist page + bbox/char offsets on
  `DocumentLine`/`DetectedSection`/`SkillEvidence` and expose them through
  `Teardown`. Cross-crate change plus optional fields for stored versions.

### Custom version dropdown (versions only)

Feasible with the already-used Radix `DropdownMenu`. A native `<select>`
supplies keyboard/mobile behavior for free, so the custom control must
explicitly implement arrow keys, type-ahead, Escape, menu roles and focus
restore. E2E coverage already exists for version switching; upload keeps its
own button beside the selector. Delete no longer belongs to this menu; reuse
the existing `AlertDialog` confirmation
flow from `web/src/components/resume-versions.tsx` in the Overview danger area
and keep the existing deletion e2e assertions.

## Implementation outline (after mockup sign-off)

1. Rebuild `web/src/routes/resumes/$id.tsx` as the version workspace: compact
   header with no grades, flat tabs in the order Overview / Structure /
   Skills / ATS insights, Overview with insight signposts + impression +
   danger area, sticky résumé rail, prominent upload button next to the version
   selector, versions-only dropdown, inline rename.
2. Add a separate Ideas route (e.g. `web/src/routes/resumes/$id/ideas.tsx`)
   reached by the call-to-action; reuse `SecondDraft` behavior inside its own
   layout with group filters and progress.
3. Reuse existing components (`ProfileSummaryCard`, `SkillCloud`,
   `ResumeStructure`, `VerdictGrid`, `ResumeViewer`, `ResumeName`,
   `ResumeVersions`) — change layout and presentation, not analysis behavior.
4. Tighten type scale and panel chrome in `web/src/styles/app.css`: one shared
   surface, ~16–18px serif detail headings, 11px mono eyebrows, 13px body;
   explicit ≤640px layout with no overflow at 390px.
5. Use nested routes for the deep links (`/resumes/$id` plus `/structure`,
   `/skills`, `/ats-insights`, `/ideas`); keep the active version in a search
   param. Preserve focus management and keyboard navigation for the tab links
   and the custom dropdown.
6. Update unit/e2e assertions that check panel CSS, heading sizes and mobile
   snapshots, then run `./scripts/check.sh`.

## Out of scope

Backend, API types, data loading, mutations, analysis pipeline, share links,
and in-document section jumps or skill highlighting.
