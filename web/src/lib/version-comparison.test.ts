import { describe, expect, it } from "vitest";

import type { ComparisonVersion } from "~/server/resumes";
import { compareVersions, comparisonMessage } from "./version-comparison";

function version(scores: (number | null)[]): ComparisonVersion {
  return { id: "version", number: 1, filename: "resume.pdf", status: "complete", created_at: "2026-11-07",
    scores: scores.map((score, index) => ({ id: String(index), name: `System ${index}`, score })) };
}

describe("version comparison", () => {
  it("summarizes increases, decreases and unchanged results", () => {
    expect(compareVersions(version([35, 70, 90]), version([60, 65, 90]))).toMatchObject({
      before: 70, after: 65, delta: -5, up: 1, down: 1, unchanged: 1, matched: 3,
    });
  });

  it("only compares matched systems and never treats missing scores as zero", () => {
    const result = compareVersions(version([0, 90, null]), version([10, null, 100]));
    expect(result).toMatchObject({ before: 0, after: 10, delta: 10, matched: 1 });
    expect(result.rows.map((row) => row.delta)).toEqual([10, null, null]);
  });

  it("matches by system ID rather than position", () => {
    const to = version([70, 40]);
    to.scores.reverse();
    expect(compareVersions(version([60, 50]), to).rows.map((row) => row.delta)).toEqual([10, -10]);
  });

  it("supports fractional medians and reverses direction when swapped", () => {
    const from = version([35, 70]);
    const to = version([95, 95]);
    expect(compareVersions(from, to).delta).toBe(42.5);
    expect(compareVersions(to, from).delta).toBe(-42.5);
  });

  it("shows no overall change until both versions have scores", () => {
    expect(compareVersions(version([90]), version([null]))).toMatchObject({
      before: null, after: null, delta: null, matched: 0,
    });
  });

  it("keeps feedback encouraging in every direction", () => {
    expect(comparisonMessage(10)).toContain("Nice progress");
    expect(comparisonMessage(-10)).toContain("not a setback");
    expect(comparisonMessage(0)).toContain("holding steady");
    expect(comparisonMessage(null)).toContain("Keep going");
  });
});
