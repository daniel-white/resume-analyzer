import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ComparisonVersion } from "~/server/resumes";
import { ResumeComparison } from "./resume-comparison";

function version(number: number, scores: ComparisonVersion["scores"]): ComparisonVersion {
  return { id: `v${number}`, number, filename: "resume.pdf", status: "complete", created_at: "2026-11-07", scores };
}

describe("comparison rows", () => {
  it("shows each system's logo and the before/after letter grades with numeric scores", () => {
    const systems = ["greenline", "levr", "workdaye", "ash", "smart_resume", "indy_hackers_hiring", "blast_track"];
    const html = renderToStaticMarkup(<ResumeComparison versionId="v2" versions={[
      version(1, systems.map((id) => ({ id, name: id, score: 77 }))),
      version(2, systems.map((id) => ({ id, name: id, score: 93 }))),
    ]} />);
    for (const id of systems) expect(html).toContain(`src="/marks/${id}.png"`);
    expect(html.match(/aria-label="Grade C\+"/g)).toHaveLength(systems.length + 1);
    expect(html.match(/aria-label="Grade A"/g)).toHaveLength(systems.length + 1);
    expect(html).toContain('comparison-numeric-score">77</span>');
    expect(html).toContain('comparison-numeric-score">93</span>');
    expect(html).toContain("Up 16 points");
  });

  it("uses a fallback icon for unknown systems and leaves missing scores pending", () => {
    const html = renderToStaticMarkup(<ResumeComparison versionId="v2" versions={[
      version(1, [{ id: "new_system", name: "New system", score: 0 }]),
      version(2, [{ id: "new_system", name: "New system", score: null }]),
    ]} />);
    expect(html).toContain("lucide-scan-text");
    expect(html).not.toContain("/marks/new_system.png");
    expect(html.match(/aria-label="Grade F"/g)).toHaveLength(1);
    expect(html).toContain("Pending");
    expect(html).toContain("Awaiting results");
    expect(html).not.toContain('aria-label="Grade null"');
  });
});
