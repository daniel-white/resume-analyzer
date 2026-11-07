import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { goodWork, letterGrade, medianScore, ScoreStamp, scoreTone } from "./score";

describe("grade feedback", () => {
  it("calculates the median without mutating scores", () => {
    expect(medianScore([])).toBeNull();
    expect(medianScore([95])).toBe(95);
    expect(medianScore([35, 70])).toBe(52.5);
    const scores = [95, 35, 70];
    expect(medianScore(scores)).toBe(70);
    expect(scores).toEqual([95, 35, 70]);
  });
  it("marks every C+ score as needing work", () => {
    for (const score of [77, 78, 79]) {
      expect(letterGrade(score)).toBe("C+");
      expect(goodWork(score)).toBe(false);
      expect(scoreTone(score)).toBe("text-score-mid");
      expect(renderToStaticMarkup(<ScoreStamp value={score} />)).toContain("Needs work");
    }
  });

  it("marks B-range grades as good work", () => {
    for (const score of [80, 83, 87, 89]) {
      expect(goodWork(score)).toBe(true);
      expect(renderToStaticMarkup(<ScoreStamp value={score} />)).toContain("Good work");
    }
  });

  it("marks the entire A range as excellent", () => {
    for (const score of [90, 93, 97, 100]) {
      expect(renderToStaticMarkup(<ScoreStamp value={score} />)).toContain("Excellent");
    }
  });
});
