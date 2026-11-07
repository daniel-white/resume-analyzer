---
name: ATS plugin
description: Add or fix a simulated ATS plugin in the resume-analyzer fleet, including asking for its name and whether to create a Python stub, and generating a fictional logo. Use when asked to add an ATS provider or plugin, when a factory wave is plugins/<id>, or when plugin factor lists disagree less than they should.
---

# Add an ATS plugin

This skill covers user-requested additions and factory repairs. It is not an agent profile. Repository paths below are relative to the repository root; `references/factor-worksheet.md` is relative to this skill directory.

For a factory wave, work on the branch supplied by the factory. Do not merge, push, or open a worktree; the factory does that. For a direct user request, follow the user's branch and delivery instructions. Do not treat permission to add a plugin as permission to deploy it.

## Ask before creating a new plugin

For an addition (not a repair), use the question tool before editing files:

1. **Should I create a stub Python provider for this plugin?** Offer **Yes, Python stub (Recommended)**, which follows the current private-provider fleet, and **No Python stub**, which adds only the Rust plugin unless the user specifies another integration.
2. **What should the simulated ATS plugin be called?** Ask for a fictional display name; the user can supply it through the tool's free-form answer. Do not silently invent the name. Derive a unique `snake_case` id that is also a valid Rust module name, and confirm the display name and id before creating files.

If the prompt already explicitly answers either question, confirm that answer and ask only what is missing. Do not ask these questions for a factory repair or when seeding the four predefined names. A real vendor integration is not a simulated plugin: stop and clarify its scope rather than adding OAuth or pretending the stub is real.

Plan an original fake logo for the confirmed name as part of every new-plugin addition. Adding a plugin through this skill includes generating its fictional logo; do not create a new plugin or image when the user only asks to edit this skill.

## Read, then stop if the contract is not real

Read in this order:

1. `docs/contracts.md`. If it still says the sections are headings only, or Verdict has no shape, stop. Do not draft `ParseInput`, the verdict, or the callback in the plugin crate.
2. `docs/plan.md`, sections "The simulated fleet" and "Event spine". Greenline, Levr, Workdaye, and Ash keep those names and biases. Do not rename them.
3. `crates/domain`. Use the plugin trait and verdict types that are defined there.
4. `crates/plugins`, `crates/app/src/providers.rs`, `crates/app/src/worker.rs`, and `providers/stub.py`. Read the registry and siblings, the raw-file handoff, how callbacks are signed, and where the HMAC secret comes from. The current fleet uses Rust registrations with private Python providers; `ATS_PROVIDERS=stub` selects the compiled stand-in.
5. The two fixture resumes the fleet tests already load. If they are not there and this is not the fleet wave seeding them, stop.
6. `web/src/components/plugin-card.tsx`, `web/public/marks`, and the active plugin legend. For a Python stub, also read `justfile`, `scripts/up.sh`, and `deploy/helm/ats-providers` before changing startup or deployment wiring.

## Invariants

These are not negotiable inside a plugin branch:

- Rust registrations are compiled in; Python stubs run as private HTTP services. No dynamic loading, vendor OAuth, or real vendor branding. Fictional names and original generated logos are required for new simulated providers; do not copy trademarked names or logos.
- A plugin does not return a verdict into the worker. It reads the raw resume file independently; the app's verification parse is not its input. Delivery is a signed `POST /api/callbacks/{plugin_id}`. Copy the sibling's delivery and the current `CallbackBody`: `run_id`, `parse`, and `verdict` (including factors). The plugin id is in the URL, not an invented body field. HMAC covers `{timestamp}.` plus the exact body bytes. Reject timestamps skewed past five minutes. A replayed idempotency key writes nothing.
- Use the same secret configuration as siblings (currently `PLUGIN_SECRET`, shared by the API and Python fleet). Never commit a production secret. If the config does not exist yet, stop and name it; do not invent incompatible per-plugin environment variables.
- Verdict fields are kept, dropped, a score from 0 to 100, a factor list, and the contract's deterministic summary. Factor advice and summary follow the existing templates, not free-form prose or a model call.
- A table drop on a PDF stays inferred (`LayoutConfidence`). Do not report it as a fact.
- Tests assert factor labels and signs, never the headline score.
- The current readable-file calibration starts at 70 (75 for Ash). Keep Python and Rust scoring in parity. Unreadable files do not receive the 20-point readable-file uplift; preserve their low failure scores. Calibration regression tests may assert score arithmetic and bounds, but provider-bias tests must still assert factor labels/signs rather than a headline grade.
- The legend subtitle says the system is simulated and modeled on publicly described parser behavior.

## Add one plugin

Do this only when at least one sibling already exists. Otherwise stop, unless the prompt is the fleet wave seeding the four.

1. Complete the name and Python-stub questions above. Read `references/factor-worksheet.md` and fill the table in the session. Do not commit the worksheet. Design a bias distinct from every sibling.
2. Use the confirmed id consistently for the Rust module, Python provider (if requested), endpoint configuration, callback path, legend id, and logo filename.
3. Add `crates/plugins/src/<id>.rs` by copying a sibling's shape, not its bias. Register it in `crates/plugins/src/lib.rs`, including the module, export, and `fleet` entry. Preserve the existing providers and compiled stand-in. Use the existing secret plumbing.
4. If requested, add the Python stub using the procedure below. If declined, do not create Python files or point the default fleet at a nonexistent service. Explain that normal fleet execution uses HTTP endpoints; Rust-only registration is not a runnable provider there. If the user wants it runnable without Python, clarify the execution path before changing worker behavior.
5. Keep a single legend. If the UI reads `GET /api/plugins`, let the registered plugin's name and subtitle supply that row. If the page still imports `web/src/mock/fleet.ts` and that file is what it renders, add the row there instead. Do not leave two lists once the API is what the page renders.
6. Generate and wire the fake logo using the procedure below.
7. Add tests beside the sibling's tests, on the designed fixture and the plain fixture. Assert labels and delta signs. The designed label set must differ from every sibling. The plain fixture must not trip the signals this plugin punishes. For a Python stub, verify its parse and factors agree with the intended Rust behavior and test signed callback delivery.
8. Run `scripts/check.sh` and the relevant provider/UI checks below. Commit on the agreed branch. In a factory wave, stop after the green gate and commit; for a direct request, deliver only as authorized by the user.

If the bias needs a layout signal that is not on `ParseInput`, stop and name the signal. Reuse the existing raw-file extraction helpers inside the provider; do not invent a new parse contract or feed it the app's verification parse.

## Add the requested Python stub

1. Follow `providers/stub.py`, not a new framework or a public vendor API. Add a `Provider(id, name, port, delay)` to `FLEET`, a distinct scoring function, and dispatch in `decide`. Reuse `read_file`, `as_parse`, `parse_job`, and `deliver`; a separate module is optional only if it fits the existing package structure.
2. Choose an unused loopback port after inspecting the existing assignments (currently 8101–8104). Keep `python3 -m providers <id>` and `python3 -m providers --all` working. Preserve `/health`, asynchronous `202` acceptance, fuzzy delay, retry behavior, and the signed callback with the run id as the idempotency key.
3. Match the worker's request: `run_id`, `file: {media_type, bytes_base64}`, and `callback_url`. Validate that the callback path belongs to this provider, enforce existing size limits, and bind locally by default. Cluster mode remains private through a ClusterIP service, never a gateway route.
4. Register the endpoint in `crates/app/src/providers.rs` if the new stub should join the default local fleet. Update tests that enumerate the fleet. Update explicit `ATS_PROVIDERS` lists in `justfile` and startup scripts as needed so they do not silently omit the provider.
5. For existing demo/cluster startup, add the id to `deploy/helm/ats-providers/values.yaml` and inspect the API chart's endpoint list and startup scripts for matching wiring. Use the existing provider image and secret injection; do not deploy or expose a service merely to add it.
6. Check `python3 -m providers --help`, start the new provider on its chosen private port, and verify `/health` returns its id. Exercise a raw-file job against a local callback receiver and verify the body, signature, and retry idempotency. Run the existing Rust registration/fleet tests and `helm template` when chart wiring changes. Stop any test servers you started.

## Existing resumes after a registry change

Every addition or repair must check a resume completed before the new plugin
was registered, not just a fresh upload. The active legend must not display a
missing run as indefinitely “Reading” without queued work. Main reconciles
missing active-plugin runs on an authenticated detail visit for the newest
parsed version, using the existing transactional `Store::fanout` and worker
outbox. Preserve completed results and older versions; never fabricate a
verdict or bypass the signed callback. A completed version must return to
`running` when a new run is queued so polling continues until its callback.

Add regression coverage for repeated/concurrent reconciliation (one run and
one request event per version/plugin), preservation of existing verdicts, and
unauthorized/unowned visits enqueueing nothing. Verify a pre-existing resume
reaches completion as well as a new upload. If repairing a live demo, inspect
stored runs and request events first: no run means registry reconciliation,
not a hung provider. Queue only the missing run through the normal outbox,
and verify its signed callback; do not delete or rerun completed runs. Deploy
or alter live data only when the user has authorized that repair. For narrow
factory scopes, report missing API/storage support rather than changing those
paths without authorization.

## Generate the fake logo

1. Inspect the existing images in `web/public/marks` for dimensions and visual style. Use the image-generation tool to create one original mark for the confirmed fictional display name. Prompt for a simple, distinctive ATS-company symbol, readable at small card size, matching the existing palette/background treatment, with no real vendor branding or resume data. Use sibling marks as style references if useful, not as designs to copy.
2. Request `web/public/marks/<id>.png` as the output path. Image generation runs asynchronously: wait for its completion notification, do not poll or repeat the request. Check the returned path and format; if necessary, convert the actual output to PNG with an available local image utility rather than merely renaming its extension.
3. Inspect the generated image. If generation is unavailable or fails, report that blocker; do not quietly omit the logo, copy another provider's, or claim it was generated.
4. Add the id to the `marked` set in `web/src/components/plugin-card.tsx` so `/marks/<id>.png` renders. Keep the image decorative (`alt=""`) beside the visible provider name, and preserve the simulated subtitle. Add fallback bias text only if needed; do not create another provider registry in the UI.
5. Run the web checks and preview the card in the browser using the available preview/browser skills. Verify that the logo loads, looks right at card size, and appears in pending and completed states without hiding the name or simulation disclosure.

## Seed the fleet

Only the fleet wave does this. Add Greenline, Levr, Workdaye, and Ash with the biases in `docs/plan.md`. The first plugin creates the registry, the callback delivery, the secret config, and the test layout. The other three copy that shape. On the designed fixture the four factor lists disagree. On the plain fixture they agree: high scores, little or nothing dropped.

The fleet test lives at `crates/plugins/tests/fleet.rs` and mentions `greenline`. Assert the factor lists, not the scores.

## Heal

The factory sends the failing gate output back into this same worktree. React to that output, not to a new design.

- `cargo fmt` or clippy names a line. Fix that line. Do not retune deltas in the same attempt.
- An assertion on a headline score is wrong. Replace it with the factor label.
- The designed factor list equals a sibling. Change this plugin's bias. Do not change the sibling, the fixture, or the assertion to hide the tie.
- You changed `crates/domain` or `docs/contracts.md` so the plugin would fit. Undo only your own changes. If the trait cannot say what you need, stop. Direct additions may update endpoint registration in `crates/app/src/providers.rs`, not redesign the worker or API contract.
- The same assertion fails three times. Stop. Report the assertion, the current factor list, and the deltas you already tried.
- A factory repair stays inside `crates/plugins` and the one active legend file unless the wave explicitly authorizes Python, endpoint/startup wiring, or logo/UI paths. If an addition requires those paths but the factory scope excludes them, stop and report the scope mismatch. Do not evade the gate. For direct additions, limit edits to the plugin, requested stub, necessary wiring, logo/card, and their tests; preserve unrelated user work.

For a factory wave, when the gate is green and the work is committed, stop. The factory rebases onto main and merges.
