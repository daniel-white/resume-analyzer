import type { ComparisonVersion } from "~/server/resumes";

import { medianScore } from "./grades";

export function compareVersions(from: ComparisonVersion, to: ComparisonVersion) {
  const rows = from.scores.map((system) => {
    const before = system.score;
    const after = to.scores.find((item) => item.id === system.id)?.score ?? null;
    return { id: system.id, name: system.name, before, after,
      delta: before !== null && after !== null ? after - before : null };
  });
  const matched = rows.filter((row) => row.delta !== null);
  const before = medianScore(matched.map((row) => row.before!));
  const after = medianScore(matched.map((row) => row.after!));
  return {
    rows, before, after,
    delta: before !== null && after !== null ? after - before : null,
    up: matched.filter((row) => row.delta! > 0).length,
    down: matched.filter((row) => row.delta! < 0).length,
    unchanged: matched.filter((row) => row.delta === 0).length,
    matched: matched.length,
  };
}

export function comparisonMessage(delta: number | null): string {
  if (delta === null) return "Your comparison will take shape as results arrive. Keep going — each revision is a chance to learn.";
  if (delta > 0) return "Nice progress! More of your résumé is getting through. Keep the changes that are working for you.";
  if (delta < 0) return "A lower score is useful feedback, not a setback. Review the systems below to find a small change to try next.";
  return "Your overall score is holding steady. Changes can still matter — check each system below to see what is working.";
}
