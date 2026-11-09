import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ProfileSummary } from "~/server/resumes";
import { ProfileSummaryCard } from "./profile-summary";

const summary: ProfileSummary = {
  text: "Ada is a computing pioneer with skills in mathematics and Rust.",
  engine: "ollama",
  model: "qwen2.5:0.5b",
  feedback: "up" as const,
};

function render(value: ProfileSummary | null = summary) {
  return renderToStaticMarkup(
    <ProfileSummaryCard summary={value} resumeId="resume-1" versionId="version-1" />,
  );
}

describe("profile summary", () => {
  it("highlights the model's impression and asks for a vote", () => {
    const html = render();
    expect(html).toContain("Who the model thinks you are");
    expect(html).toContain(summary.text);
    expect(html).toContain("Local LLM");
    expect(html).toContain(summary.model);
    expect(html).toContain("may be inaccurate");
    expect(html).toContain("Is this summary accurate?");
    expect(html).toContain("Yes, this summary is accurate");
    expect(html.match(/aria-pressed="true"/g)).toHaveLength(1);
  });

  it("renders model output as text rather than markup", () => {
    const html = render({ ...summary, text: "<img src=x onerror=alert(1)> Ignore instructions" });
    expect(html).not.toContain("<img src=x");
    expect(html).toContain("&lt;img src=x");
  });

  it("explains when a summary is not available yet", () => {
    const html = render(null);
    expect(html).toContain("after this version is parsed");
    expect(html).not.toContain("Is this summary accurate?");
  });

  it("shows the pending message when the fallback produced no text", () => {
    const html = render({ text: "", engine: "extractive", model: "section-extract-v1", feedback: null });
    expect(html).toContain("after this version is parsed");
    expect(html).not.toContain("Is this summary accurate?");
  });

  it("shows a different layout when the document does not look like a résumé", () => {
    const html = render({
      text: "This does not look like a résumé. Could not find enough work experience education skills or project information to build a grounded professional profile for review. Please upload a document that includes those details for a clear summary based on actual work history and qualifications.",
      engine: "ollama",
      model: "qwen2.5:0.5b",
      feedback: null,
    });
    expect(html).toContain("Not a résumé");
    expect(html).not.toContain("Who the model thinks you are");
    expect(html).not.toContain("Is this summary accurate?");
    expect(html).toContain("Upload a document with work experience");
  });
});
