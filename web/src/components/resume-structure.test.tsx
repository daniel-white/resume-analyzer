import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { SectionMap } from "~/server/resumes";
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

describe("our reading", () => {
  it("lists each detected section as a selectable control", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure sections={sections} fields={[{ label: "Name", value: "Ada Lovelace" }]} resumeId="resume-1" versionId="version-1" onHighlight={() => {}} />,
    );
    expect(html).toContain("Résumé details");
    expect(html).toContain("Ada Lovelace");
    expect(html).toContain("Experience");
    expect(html).toContain("Education");
    expect(html).toContain("94%");
    expect(html).toContain('aria-expanded="false"');
  });

  it("shows section body and vote controls when selected", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure
        sections={sections}
        resumeId="resume-1"
        versionId="version-1"
        onHighlight={() => {}}
        initialSelectedId="experience-1"
      />,
    );
    expect(html).toContain("Built things.");
    expect(html).toContain("Did we read this section correctly?");
    expect(html).not.toContain("Which systems found it");
  });

  it("shows a reading state before sections arrive", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure sections={null} resumeId="resume-1" versionId="version-1" onHighlight={() => {}} />,
    );
    expect(html).toContain("Reading your file into sections.");
  });

  it("renders selected section with aria-expanded true when initialSelectedId matches", () => {
    const html = renderToStaticMarkup(
      <ResumeStructure
        sections={sections}
        resumeId="resume-1"
        versionId="version-1"
        onHighlight={() => {}}
        initialSelectedId="experience-1"
      />,
    );
    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain("structure-body-experience-1");
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
