import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ChecklistItem } from "~/server/resumes";
import { SecondDraft } from "./second-draft";

const items: ChecklistItem[] = [
  {
    id: "item-blank",
    fingerprint: "fp-blank",
    kind: "blank",
    title: "Add a number to Triage",
    detail: "Your line: \"Triaged patients arriving in the busy emergency department.\"",
    why: "A duty rather than a result.",
    placeholder: "how many triage · how often · what changed",
    section_id: "experience-1",
    section_heading: "Experience",
    quote: "Triaged patients arriving in the busy emergency department.",
    template: "Triaged patients — ___",
    cut_text: null,
    position: 0,
    done: false,
    answer: null,
    checked_at: null,
    created_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "item-promote",
    fingerprint: "fp-promote",
    kind: "promote",
    title: "Bring Escalations up top",
    detail: "Your best proof sits in Skills.",
    why: "One of your most substantiated claims.",
    placeholder: "move it to your summary",
    section_id: "skills-1",
    section_heading: "Skills",
    quote: "Resolved more than two hundred escalations every quarter.",
    template: "Resolved escalations — ___",
    cut_text: null,
    position: 1,
    done: true,
    answer: "200 each quarter",
    checked_at: "2026-01-02T00:00:00Z",
    created_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "item-cut",
    fingerprint: "fp-cut",
    kind: "cut",
    title: "Cut fluff",
    detail: "Found in Summary.",
    why: "An unsupported trait.",
    placeholder: "what result will you put in its place?",
    section_id: "summary-1",
    section_heading: "Summary",
    quote: null,
    template: null,
    cut_text: "Hard-working team player with a passion for excellence.",
    position: 2,
    done: false,
    answer: null,
    checked_at: null,
    created_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "item-flag",
    fingerprint: "fp-flag",
    kind: "flag",
    title: "Flag: Blaming a former manager",
    detail: "Found in Experience: \"My old manager never explained the process.\"",
    why: "It puts the focus on a person instead of your work.",
    placeholder: "Describe the gap without naming anyone.",
    section_id: "experience-1",
    section_heading: "Experience",
    quote: "My old manager never explained the process.",
    template: null,
    cut_text: null,
    position: 3,
    done: false,
    answer: null,
    checked_at: null,
    created_at: "2026-01-01T00:00:00Z",
  },
];

function render(value: ChecklistItem[] = items, pending = false) {
  return renderToStaticMarkup(
    <SecondDraft
      items={value}
      resumeId="resume-1"
      versionId="version-1"
      pending={pending}
    />,
  );
}

describe("second draft", () => {
  it("sits in the same surrounding box as the other live panels", () => {
    expect(render()).toContain('class="second-draft"');
    const css = readFileSync(
      fileURLToPath(new URL("../styles/app.css", import.meta.url)),
      "utf8",
    );
    const rule = css.match(/\.second-draft \{[^}]*\}/)?.[0] ?? "";
    for (const property of ["padding:", "border: 1px", "border-radius:", "background:", "box-shadow:"]) {
      expect(rule, `missing ${property} on .second-draft`).toContain(property);
    }
  });

  it("groups every kind under its own heading", () => {
    const html = render();
    expect(html).toContain("Fill the blanks");
    expect(html).toContain("Bring up the gold");
    expect(html).toContain("Cut the fluff");
    expect(html).toContain("Flag bad practices");
    expect(html).toContain('data-kind="blank"');
    expect(html).toContain('data-kind="promote"');
    expect(html).toContain('data-kind="cut"');
    expect(html).toContain('data-kind="flag"');
  });

  it("shows a progress count and bar for the checked items", () => {
    const html = render();
    expect(html).toContain("1 of 4 done");
    expect(html).toContain("width:25%");
    expect(html.match(/checked=""/g)).toHaveLength(1);
  });

  it("offers an optional answer box with the item's own hint", () => {
    const html = render();
    expect(html).toContain('placeholder="how many triage · how often · what changed"');
    expect(html).toContain("Your note for: Add a number to Triage");
  });

  it("fills the template hole with the saved answer", () => {
    const html = render();
    expect(html).toContain("second-draft-fill");
    expect(html).toContain("200 each quarter");
    expect(html).toContain("Resolved escalations — ");
  });

  it("keeps the exact line to cut and labels its source section", () => {
    const html = render();
    expect(html).toContain("Hard-working team player with a passion for excellence.");
    expect(html).toContain("From Experience");
  });

  it("shows the model's flag with its own rewrite hint", () => {
    const html = render();
    expect(html).toContain("Flag: Blaming a former manager");
    expect(html).toContain("My old manager never explained the process.");
    expect(html).toContain('placeholder="Describe the gap without naming anyone."');
  });

  it("keeps an encouraging waiting state while the checklist is built", () => {
    const html = render([], true);
    expect(html).toContain("Building your checklist");
    expect(html).not.toContain("Fill the blanks");
  });
});
