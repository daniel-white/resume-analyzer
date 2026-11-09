import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { SectionMap, VerdictCard } from "~/server/resumes";
import { ResumeStructure, VoteButtons } from "./resume-structure";

const sections: SectionMap = {
  engine: "fastembed",
  model: "bge-small",
  sections: [
    {
      id: "experience-1",
      kind: "experience",
      heading: "Experience",
      body: "Built things.",
      confidence: 0.94,
      feedback: null,
    },
    {
      id: "education-1",
      kind: "education",
      heading: "Education",
      body: "Learned things.",
      confidence: 0.9,
      feedback: null,
    },
  ],
};

const cards: VerdictCard[] = [
  {
    id: "greenline",
    name: "Greenline",
    subtitle: "Standard headings.",
    pending: false,
    score: 80,
    band: "high",
    kept: ["Experience"],
    missed: [],
    factors: [],
    parse: null,
  },
  {
    id: "levr",
    name: "Levr",
    subtitle: "Exact tokens.",
    pending: false,
    score: 70,
    band: "mid",
    kept: ["email"],
    missed: [],
    factors: [],
    parse: null,
  },
  {
    id: "ash",
    name: "Ash",
    subtitle: "Still reading.",
    pending: true,
  },
];

describe("our reading", () => {
  it("lists each detected section as a selectable control", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure sections={sections} cards={cards} resumeId="resume-1" versionId="version-1" onHighlight={() => {}} />,
    );
    expect(html).toContain("Our read of your resume");
    expect(html).toContain("Experience");
    expect(html).toContain("Education");
    expect(html).toContain("94%");
    expect(html).toContain('aria-expanded="false"');
  });

  it("compares which systems found the selected section", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure
        sections={sections}
        cards={cards}
        resumeId="resume-1"
        versionId="version-1"
        onHighlight={() => {}}
        initialSelectedId="experience-1"
      />,
    );
    expect(html).toContain("Built things.");
    expect(html).toContain("Did we read this section correctly?");
    expect(html).toContain("Which systems found it");
    expect(html).toContain("Greenline");
    expect(html).toContain("Levr");
    expect(html).not.toContain("Ash");
    expect(html).toContain(">Found<");
    expect(html).toContain(">Missed<");
  });

  it("shows a reading state before sections arrive", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure sections={null} cards={cards} resumeId="resume-1" versionId="version-1" onHighlight={() => {}} />,
    );
    expect(html).toContain("Reading your file into sections.");
  });
});

describe("section votes", () => {
  function render(feedback: "up" | "down" | null) {
    return renderToStaticMarkup(
      <VoteButtons feedback={feedback} busy={false} onVote={() => {}} />,
    );
  }

  it("fills only the selected thumb", () => {
    expect(render(null)).not.toContain('fill="currentColor"');
    const up = render("up");
    expect(up.match(/fill="currentColor"/g)).toHaveLength(1);
    expect(up).toContain('aria-pressed="true"');
    expect(up.match(/aria-pressed="true"/g)).toHaveLength(1);
    const down = render("down");
    expect(down.match(/fill="currentColor"/g)).toHaveLength(1);
    expect(down.match(/aria-pressed="true"/g)).toHaveLength(1);
  });
});
