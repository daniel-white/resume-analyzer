export function medianScore(scores: number[]): number | null {
  if (scores.length === 0) return null;
  const sorted = [...scores].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  const upper = sorted[middle];
  if (upper === undefined) return null;
  if (sorted.length % 2) return upper;
  const lower = sorted[middle - 1];
  if (lower === undefined) return null;
  return (lower + upper) / 2;
}
