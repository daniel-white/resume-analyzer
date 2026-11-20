import assert from "node:assert/strict";
import test from "node:test";

import {
  assertCleanMarkdown,
  draftMarkdown,
  normalizeMarkdown,
  parseChangelog,
  parseRewriteResponse,
  rewriteMarkdown,
  rewritePromptPayload,
  stripRefs,
} from "./release-notes.mjs";

const changelog = `# Changelog

## [1.2.0](https://example.com/compare/v1.1.0...v1.2.0) (2026-11-19)

### Features

* stream analysis to the UI ([#7](https://example.com/issues/7)) ([abc1234](https://example.com/commit/abc1234))
* bump pnpm to 9 ([#8](https://example.com/issues/8)) ([def5678](https://example.com/commit/def5678))

### Bug Fixes

* keep the upload button visible on mobile ([1234567](https://example.com/commit/1234567))
* cache cargo builds in actions ([7654321](https://example.com/commit/7654321))

### Chores

* update dependencies ([#11](https://example.com/issues/11))

## [1.1.0](https://example.com/compare/v1.0.0...v1.1.0) (2026-11-01)

### Features

* an older feature
`;

test("parseChangelog keeps user-facing sections and strips refs", () => {
  const draft = parseChangelog(changelog, "1.2.0");
  assert.deepEqual(draft, {
    version: "1.2.0",
    sections: [
      {
        title: "Features",
        items: ["stream analysis to the UI"],
      },
      {
        title: "Fixes",
        items: ["keep the upload button visible on mobile"],
      },
    ],
  });
});

test("parseChangelog drops internal commit scopes and strips user-facing ones", () => {
  const scoped = `# Changelog\n\n## [2.0.0](https://example.com) (2026-11-20)\n\n### Features\n\n* **about:** add a changelog tab\n\n### Bug Fixes\n\n* **release-notes:** strip model JSON wrappers\n`;
  assert.deepEqual(parseChangelog(scoped, "2.0.0"), {
    version: "2.0.0",
    sections: [{ title: "Features", items: ["add a changelog tab"] }],
  });
});

test("parseChangelog fails clearly for an unknown version", () => {
  assert.throws(() => parseChangelog(changelog, "9.9.9"), /no section for version 9\.9\.9/);
});

test("stripRefs removes links, PR numbers, and hashes", () => {
  assert.equal(
    stripRefs("authenticate cosign ([#14](https://example.com/issues/14)) ([1b8a332](https://example.com/commit/1b8a332))"),
    "authenticate cosign",
  );
});

test("draft markdown has a stable empty form", () => {
  assert.equal(
    draftMarkdown({ version: "1.2.0", sections: [] }),
    "## Version 1.2.0\n\nNo user-facing changes in this release.\n",
  );
});

test("normalizeMarkdown canonicalizes LLM edits and drops unsafe content", () => {
  const edited = `## Version 9.9.9

Analysis now streams into the page, so feedback arrives sooner.

### Bug Fixes

- The upload button stays visible on mobile ([#21](https://example.com/issues/21))

### Chores

- rename a variable
`;
  assert.equal(
    normalizeMarkdown(edited, "1.2.0"),
    "## Version 1.2.0\n\nAnalysis now streams into the page, so feedback arrives sooner.\n\n### Fixes\n\n- The upload button stays visible on mobile\n",
  );
});

test("a user-facing bullet containing the word build is kept", () => {
  const edited = `## Version 1.2.0\n\n### Features\n\n- Build a clearer résumé in fewer steps\n`;
  assert.equal(
    normalizeMarkdown(edited, "1.2.0"),
    "## Version 1.2.0\n\n### Features\n\n- Build a clearer résumé in fewer steps\n",
  );
});

test("rewritePromptPayload numbers every changelog item", () => {
  const draft = parseChangelog(changelog, "1.2.0");
  assert.equal(
    rewritePromptPayload(draft),
    "[Features]\n- (0) stream analysis to the UI\n\n[Fixes]\n- (1) keep the upload button visible on mobile",
  );
});

test("parseRewriteResponse accepts fenced JSON and rejects prose", () => {
  const reply = '```json\n{"summary": "A summary.", "rewrites": {"0": "A rewrite."}}\n```';
  assert.deepEqual(parseRewriteResponse(reply), {
    summary: "A summary.",
    rewrites: { 0: "A rewrite." },
  });
  assert.throws(() => parseRewriteResponse("here are the notes"), /did not return a JSON object/);
});

test("rewriteMarkdown keeps every changelog item even if the model omits one", () => {
  const draft = parseChangelog(changelog, "1.2.0");
  const notes = normalizeMarkdown(
    rewriteMarkdown(draft, "Feedback now arrives as analysis runs.", { 0: "Analysis streams into the page as it runs." }),
    "1.2.0",
  );
  assert.equal(
    notes,
    "## Version 1.2.0\n\nFeedback now arrives as analysis runs.\n\n### Features\n\n- Analysis streams into the page as it runs.\n\n### Fixes\n\n- keep the upload button visible on mobile\n",
  );
  assertCleanMarkdown(notes);
});

test("rewriteMarkdown falls back to the draft when a rewrite is internal", () => {
  const draft = parseChangelog(changelog, "1.2.0");
  const notes = normalizeMarkdown(
    rewriteMarkdown(draft, "", { 0: "checksums are signed in the release job" }),
    "1.2.0",
  );
  assert.match(notes, /- stream analysis to the UI/);
  assert.doesNotMatch(notes, /checksums/);
});

test("assertCleanMarkdown rejects JSON or code-fence artifacts", () => {
  assert.throws(() => assertCleanMarkdown('{"markdown": "notes"}'), /JSON wrapper|code fence/);
  assert.throws(() => assertCleanMarkdown("```markdown\nnotes\n```"), /JSON wrapper|code fence/);
  assert.doesNotThrow(() => assertCleanMarkdown("## Version 0.3.0\n\nClean notes.\n"));
});

test("normalizeMarkdown keeps section order independent of the input", () => {
  const edited = `## Version 1.2.0

### Fixes

- A fix users can see

### Features

- A feature users can see
`;
  assert.equal(
    normalizeMarkdown(edited, "1.2.0"),
    "## Version 1.2.0\n\n### Features\n\n- A feature users can see\n\n### Fixes\n\n- A fix users can see\n",
  );
});
