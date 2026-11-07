import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { VerdictCardView } from "./verdict-card";
import { pluginCards, type ProviderReport, type RunView } from "~/server/resumes";

const report: ProviderReport = {
  engine: "Plaintext Classic",
  engine_version: "3.8.2",
  reference: "GREENLINE-ABC123",
  outcome: "parsed_with_warnings",
  contact_fields: 2,
  sections_detected: 1,
  bullets_detected: 3,
  layout_confidence: "inferred",
  notes: ["PDF inspection is limited to embedded text."],
};

function markup(reportValue?: ProviderReport) {
  const run: RunView = {
    id: "run-1", plugin_id: "greenline", status: "complete", parse: null,
    verdict: { score: 70, kept: ["email"], dropped: ["table"], summary: "Score 70 of 100.", factors: [], report: reportValue },
  };
  const cards = pluginCards([{ id: "greenline", name: "Greenline", subtitle: "Simulated parser" }], [run]);
  return renderToStaticMarkup(<VerdictCardView card={cards[0]} initiallySelected />);
}

describe("provider report delivery to the card", () => {
  it("renders backend diagnostics with simulation and inference disclosures", () => {
    const html = markup(report);
    for (const text of ["Simulated parser report", "Parsed with warnings", "Plaintext Classic", "v3.8.2", "GREENLINE-ABC123", "2 / 3", "PDF inference", report.notes[0], "not a guarantee"]) {
      expect(html).toContain(text);
    }
  });

  it("keeps old verdicts usable without inventing a report", () => {
    const html = markup();
    expect(html).toContain("Score 70 of 100.");
    expect(html).not.toContain("Simulated parser report");
  });

  it("never displays extraction metrics on a failed read", () => {
    const html = markup({ ...report, outcome: "unreadable", notes: ["Could not open file."] });
    expect(html).toContain("Could not read file");
    expect(html).not.toContain("Contact fields detected");
  });
});
