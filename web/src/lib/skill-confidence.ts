/**
 * Low-confidence skills stay in storage for auditing, but they are not evidence
 * worth showing. The cut is inclusive: 20% is presented, 19% is not.
 */
export const MIN_PRESENTED_CONFIDENCE = 20;

export function isPresentableConfidence(confidence: number): boolean {
  return Number.isFinite(confidence) && confidence >= MIN_PRESENTED_CONFIDENCE;
}
