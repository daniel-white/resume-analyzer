#!/usr/bin/env node
// Deterministic release notes for the About page.
//
// Flow: CHANGELOG.md -> filtered markdown draft -> (LLM-edited markdown in
// release-notes/<version>.md) -> web/src/generated/release-notes.md -> React.
//
// The LLM only writes the editable markdown file. Everything that reaches the
// app passes through normalizeMarkdown below, so hashes, PR numbers, links,
// and internal sections cannot leak back in, and `check` is byte-for-byte.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const changelogPath = join(root, "CHANGELOG.md");
const versionPath = join(root, "version.txt");
const notesDirectory = join(root, "release-notes");
const artifactPath = join(root, "web/src/generated/release-notes.md");
const promptPath = join(root, "scripts/release-notes.prompt.md");

const EMPTY_NOTES = "No user-facing changes in this release.";

// Only these release-please sections describe something a user can see.
const sectionNames = new Map([
  ["features", "Features"],
  ["feature", "Features"],
  ["bug fixes", "Fixes"],
  ["fixes", "Fixes"],
  ["performance improvements", "Performance"],
  ["performance", "Performance"],
]);

// Other release-please sections are always dropped. This second pass catches
// internal work that release-please filed under Features or Bug Fixes.
const internalWork =
  /\b(?:ci|github actions|workflow|pipeline|buildx|imagetools|docker|helm|charts?|pnpm|cargo|dependenc(?:y|ies)|dependabot|lockfile|e2e|tests?|release-please|release jobs?|release pipeline|release workflow|cosign|ghcr|signing|signed|attestations?|checksums?|registry|build metadata|bake|cache)\b/i;

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function stripRefs(text) {
  return text
    .replace(/\(\s*\[#\d+\]\([^)]*\)\s*\)/g, "")
    .replace(/\(\s*\[[0-9a-f]{7,40}\]\([^)]*\)\s*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\(\s*(?:#\d+|[0-9a-f]{7,40})\s*\)/g, "")
    .replace(/\b(?:#\d+|[0-9a-f]{7,40})\b/gi, "")
    .replace(/\[\s*\]/g, "")
    .replace(/\(\s*\)/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

export function readVersion() {
  const version = readFileSync(versionPath, "utf8").trim();
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`version.txt must contain a semver version, got ${JSON.stringify(version)}`);
  }
  return version;
}

export function parseChangelog(markdown, version) {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const header = new RegExp(`^##\\s+\\[?${escapeRegExp(version)}\\]?\\b`);
  const start = lines.findIndex((line) => header.test(line));
  if (start === -1) {
    throw new Error(`CHANGELOG.md has no section for version ${version}`);
  }

  const body = [];
  for (const line of lines.slice(start + 1)) {
    if (/^##\s+\[?\d/.test(line)) break;
    body.push(line);
  }

  const sections = [];
  let current = null;
  for (const line of body) {
    const heading = /^###\s+(.+?)\s*$/.exec(line);
    if (heading) {
      const title = sectionNames.get(heading[1].trim().toLowerCase());
      current = title === undefined ? null : { title, items: [] };
      if (current) sections.push(current);
      continue;
    }
    if (!current) continue;

    const bullet = /^\s*[-*+]\s+(.+)$/.exec(line);
    if (bullet) {
      current.items.push(bullet[1].trim());
    } else if (/^\s{2,}\S/.test(line) && current.items.length > 0) {
      current.items[current.items.length - 1] += ` ${line.trim()}`;
    }
  }

  for (const section of sections) {
    section.items = section.items
      .map(stripRefs)
      .filter((item) => item.length > 0 && !internalWork.test(item));
  }

  return {
    version,
    sections: sections.filter((section) => section.items.length > 0),
  };
}

export function draftMarkdown(draft) {
  const lines = [`## Version ${draft.version}`, ""];
  if (draft.sections.length === 0) {
    lines.push(EMPTY_NOTES);
    return `${lines.join("\n")}\n`;
  }
  for (const section of draft.sections) {
    lines.push(`### ${section.title}`, "");
    for (const item of section.items) {
      lines.push(`- ${item}`);
    }
    lines.push("");
  }
  return `${lines.join("\n").trimEnd()}\n`;
}

// Turn the editable markdown (draft or LLM output) into one canonical form.
// Unknown headings, links, hashes, and PR numbers are removed here, not in the
// UI, so the committed artifact is always safe to render as HTML.
export function normalizeMarkdown(markdown, version) {
  const lines = markdown
    .replace(/\r\n?/g, "\n")
    .replace(/```[\s\S]*?```/g, "")
    .split("\n");
  const paragraphs = [];
  const items = new Map([
    ["Features", []],
    ["Fixes", []],
    ["Performance", []],
  ]);
  let paragraph = [];

  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    const text = stripRefs(paragraph.join(" "));
    if (text.length > 0 && !internalWork.test(text)) paragraphs.push(text);
    paragraph = [];
  };

  let current = null;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.length === 0) {
      flushParagraph();
      continue;
    }
    // A quoted JSON wrapper or code fence is never release-note content. Drop
    // it here as a second line of defence after unwrapModelContent.
    if (/^[{[]/.test(line) || /^"(?:markdown|content)"\s*:/.test(line)) {
      flushParagraph();
      continue;
    }
    const versionHeading = /^##\s+Version\b/i.test(line);
    const sectionHeading = /^###\s+(.+?)\s*$/.exec(line);
    if (versionHeading || /^#{1,6}\s/.test(line)) {
      flushParagraph();
      current =
        sectionHeading === null ? null : (sectionNames.get(sectionHeading[1].trim().toLowerCase()) ?? null);
      continue;
    }
    const bullet = /^[-*+]\s+(.+)$/.exec(line);
    if (bullet) {
      flushParagraph();
      if (current !== null) {
        const text = stripRefs(bullet[1]);
        if (text.length > 0 && !internalWork.test(text)) {
          items.get(current).push(text);
        }
      }
      continue;
    }
    paragraph.push(line);
  }
  flushParagraph();

  const output = [`## Version ${version}`, ""];
  if (paragraphs.length === 0 && [...items.values()].every((list) => list.length === 0)) {
    output.push(EMPTY_NOTES);
    return `${output.join("\n")}\n`;
  }
  if (paragraphs.length > 0) output.push(...paragraphs, "");
  for (const [title, list] of items) {
    if (list.length === 0) continue;
    output.push(`### ${title}`, "");
    for (const item of list) output.push(`- ${item}`);
    output.push("");
  }
  return `${output.join("\n").trimEnd()}\n`;
}

function curatedPath(version) {
  return join(notesDirectory, `${version}.md`);
}

// A bad model response must fail the release job, not ship as release notes.
export function assertCleanMarkdown(markdown) {
  if (/```/.test(markdown) || /^[{[]/m.test(markdown) || /\\n/.test(markdown)) {
    throw new Error("release notes contain a JSON wrapper or code fence");
  }
}

export function expectedMarkdown(version) {
  const markdown = readFileSync(changelogPath, "utf8");
  const draft = parseChangelog(markdown, version);
  let source = draftMarkdown(draft);
  try {
    source = readFileSync(curatedPath(version), "utf8");
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const notes = normalizeMarkdown(source, version);
  assertCleanMarkdown(notes);
  return notes;
}

function writeOutput(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function parseModelResponse(raw) {
  const fenced = raw.trim().replace(/^```(?:markdown|md)?\s*/i, "").replace(/```$/, "");
  try {
    return JSON.parse(fenced);
  } catch {
    const start = fenced.indexOf("{");
    const end = fenced.lastIndexOf("}");
    if (start === -1 || end <= start) {
      throw new Error("the model did not return a JSON object");
    }
    return JSON.parse(fenced.slice(start, end + 1));
  }
}

// Some chat models return the requested JSON object *as the message text*
// instead of as the completion payload. Accept both shapes: plain markdown or
// a `{"markdown": "..."}` wrapper, with or without a code fence.
export function unwrapModelContent(content) {
  let text = content.trim();
  const fenced = /^```(?:markdown|md|json)?\s*([\s\S]*?)```$/i.exec(text);
  if (fenced) text = fenced[1].trim();

  const candidates = [text];
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start !== -1 && end > start) candidates.push(text.slice(start, end + 1));
  for (const candidate of candidates) {
    try {
      const parsed = JSON.parse(candidate);
      if (parsed && typeof parsed.markdown === "string") return parsed.markdown;
    } catch {
      // Plain markdown, or a JSON object that is not a wrapper.
    }
  }
  return text;
}

async function editWithModel(version) {
  const endpoint = process.env["RELEASE_NOTES_LLM_URL"] ?? "http://127.0.0.1:8080/v1/chat/completions";
  const model = process.env["RELEASE_NOTES_LLM_MODEL"] ?? "llama-3.2-3b-instruct-q4_k_m";
  const draft = draftMarkdown(parseChangelog(readFileSync(changelogPath, "utf8"), version));
  const prompt = readFileSync(promptPath, "utf8")
    .replaceAll("{{VERSION}}", version)
    .replaceAll("{{DRAFT}}", draft);

  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      model,
      messages: [{ role: "system", content: prompt }],
      temperature: 0,
      seed: 1337,
      max_tokens: 1200,
      stream: false,
    }),
    signal: AbortSignal.timeout(180_000),
  });
  if (!response.ok) {
    throw new Error(`model request failed (${response.status}): ${(await response.text()).slice(0, 500)}`);
  }

  const payload = parseModelResponse(await response.text());
  const content = payload?.choices?.[0]?.message?.content;
  if (typeof content !== "string" || content.trim().length === 0) {
    throw new Error("the model returned an empty response");
  }
  const notes = normalizeMarkdown(unwrapModelContent(content), version);
  assertCleanMarkdown(notes);
  return notes;
}

async function main() {
  const command = process.argv[2] ?? "generate";
  const version = readVersion();

  switch (command) {
    case "generate": {
      const markdown = expectedMarkdown(version);
      writeOutput(artifactPath, markdown);
      process.stdout.write(markdown);
      break;
    }
    case "check": {
      let actual;
      try {
        actual = readFileSync(artifactPath, "utf8");
      } catch {
        throw new Error(`${artifactPath} is missing; run node scripts/release-notes.mjs generate`);
      }
      const expected = expectedMarkdown(version);
      if (actual !== expected) {
        throw new Error(
          `${artifactPath} is stale. Run node scripts/release-notes.mjs generate and commit the result.`,
        );
      }
      process.stdout.write(`release notes: ${version} up to date\n`);
      break;
    }
    case "draft": {
      process.stdout.write(draftMarkdown(parseChangelog(readFileSync(changelogPath, "utf8"), version)));
      break;
    }
    case "edit": {
      const markdown = await editWithModel(version);
      writeOutput(curatedPath(version), markdown);
      writeOutput(artifactPath, markdown);
      process.stdout.write(markdown);
      break;
    }
    default:
      throw new Error(`unknown command ${JSON.stringify(command)}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`release-notes: ${error.message}`);
    process.exitCode = 1;
  });
}
