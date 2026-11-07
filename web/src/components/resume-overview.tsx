import { CheckCheck, FileText, ScanLine, Sparkles } from "lucide-react";

import type { VerdictCard } from "~/server/resumes";

export function ResumeOverview({ filename, lede, cards, number }: {
  filename: string;
  lede: string;
  cards: VerdictCard[];
  number: number;
}) {
  const scores = cards.flatMap((card) => card.pending ? [] : [card.score]);
  const complete = cards.length > 0 && scores.length === cards.length;
  const spread = scores.length ? `${Math.min(...scores)}–${Math.max(...scores)}` : "—";

  return (
    <header className="resume-overview">
      <div className="overview-title">
        <span className="overview-icon" aria-hidden="true"><FileText /></span>
        <div>
          <p className="eyebrow">Resume field notes · v{number}</p>
          <h1 className="filename">{filename}</h1>
          <p className="lede">{lede}</p>
        </div>
      </div>
      <div className="overview-stats" aria-label="Analysis overview">
        <div><span className="stat-label"><ScanLine aria-hidden="true" /> Independent reads</span><strong>{scores.length}<small> / {cards.length}</small></strong></div>
        <div><span className="stat-label"><Sparkles aria-hidden="true" /> Score range</span><strong>{spread}<small> / 100</small></strong></div>
        <p className={`analysis-state ${complete ? "is-complete" : ""}`} role="status">
          {complete ? <CheckCheck aria-hidden="true" /> : <ScanLine aria-hidden="true" />}
          {complete ? "Analysis complete" : "Analysis in progress"}
        </p>
      </div>
    </header>
  );
}
