import { cn } from "~/lib/utils";

export function letterGrade(score: number): string {
  if (score >= 97) return "A+";
  if (score >= 93) return "A";
  if (score >= 90) return "A-";
  if (score >= 87) return "B+";
  if (score >= 83) return "B";
  if (score >= 80) return "B-";
  if (score >= 77) return "C+";
  if (score >= 73) return "C";
  if (score >= 70) return "C-";
  if (score >= 67) return "D+";
  if (score >= 63) return "D";
  if (score >= 60) return "D-";
  return "F";
}

export function scoreTone(score: number): string {
  if (score >= 77) return "text-score-high";
  if (score >= 60) return "text-score-mid";
  return "text-score-low";
}

export function goodWork(score: number): boolean {
  return score >= 77;
}

export function ScoreStamp({ value }: { value: number }) {
  const good = goodWork(value);
  return (
    <span className={good ? "ink-stamp ink-stamp-good" : "ink-stamp ink-stamp-work"}>
      {good ? "Good work" : "Needs work"}
    </span>
  );
}

export function Score({ value, compact = false }: { value: number; compact?: boolean }) {
  const grade = letterGrade(value);
  return (
    <span
      className={cn(
        "font-serif leading-none font-medium",
        compact ? "text-[1.65rem]" : "text-[56px]",
        scoreTone(value),
      )}
      aria-label={`Grade ${grade}`}
    >
      {grade}
    </span>
  );
}
