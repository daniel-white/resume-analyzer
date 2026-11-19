import assert from "node:assert/strict";
import test from "node:test";

import { draftMarkdown, normalizeMarkdown, parseChangelog, stripRefs } from "./release-notes.mjs";

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
