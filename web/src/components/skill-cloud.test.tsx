import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { SectionMap, SkillSet } from "~/server/resumes";
import { SkillCloud } from "./skill-cloud";

const sections: SectionMap = {
  engine: "sections-v1",
  model: "test",
  sections: [
    {
      id: "experience-1",
      kind: "experience",
      heading: "Experience",
      body: "Taught patients and coordinated care.",
      confidence: 0.9,
      feedback: null,
    },
  ],
};

const skills: SkillSet = {
  engine: "skills-v1",
  model: "test",
  skills: [
    {
      name: "Patient education",
      canonical: "patient-education",
      category: "practice",
      nature: "people",
      importance: 90,
      confidence: 88,
      confidence_reason: "Grounded in experience.",
      evidence: [{ section_id: "experience-1", quote: "Taught patients" }],
    },
    {
      name: "Care coordination",
      canonical: "care-coordination",
      category: "method",
      nature: "hard",
      importance: 60,
      confidence: 70,
      confidence_reason: "Named in the skills section.",
      evidence: [{ section_id: "skills-1", quote: "Coordination" }],
    },
  ],
};

function cloud() {
  return renderToStaticMarkup(
    <SkillCloud
      skills={skills}
      sections={sections}
      versionId="version-1"
    />,
  );
}

describe("skill cloud", () => {
  it("sits in the same surrounding box as the model impression", () => {
    expect(cloud()).toContain('class="skill-cloud"');
    const css = readFileSync(
      fileURLToPath(new URL("../styles/app.css", import.meta.url)),
      "utf8",
    );
    const rule = css.match(/\.skill-cloud \{[^}]*\}/)?.[0] ?? "";
    for (const property of ["padding:", "border: 1px", "border-radius:", "background:", "box-shadow:"]) {
      expect(rule, `missing ${property} on .skill-cloud`).toContain(property);
    }
  });

  it("renders one tag per stored skill with both taxonomy axes", () => {
    const html = cloud();
    expect(html).toContain('data-testid="skill-tag"');
    expect(html).toContain('data-nature="people"');
    expect(html).toContain('data-nature="hard"');
    expect(html).toContain("skill-tag--practice");
    expect(html).toContain("skill-tag--method");
    expect(html).toContain("importance 90 of 100");
  });

  it("offers a contextual filter for every nature with counts", () => {
    const html = cloud();
    expect(html).toContain('aria-label="Filter skills by nature"');
    expect(html).toContain("Technical");
    expect(html).toContain("People");
    expect(html).toContain("skill-filter-count");
    expect(html).toContain('aria-pressed="true"');
  });

  it("warns when a skill is supported only by a list entry", () => {
    expect(cloud()).toContain("Needs a concrete example: Care coordination");
  });

  it("keeps an encouraging waiting state when nothing is stored yet", () => {
    const html = renderToStaticMarkup(
      <SkillCloud
        skills={null}
        sections={null}
        versionId="version-1"
        pending
      />,
    );
    expect(html).toContain("Tags will appear here one at a time");
  });
});
