import { CheckCheck, FileText, ScanLine, Sparkles } from "lucide-react";

import type { VerdictCard } from "~/server/resumes";
import { letterGrade, medianScore, ScoreStamp } from "./score";
import { ResumeName } from "./resume-name";

export function ResumeOverview({ resumeId, title, filename, cards, number }: {
  resumeId: string;
  title: string;
  filename: string;
  cards: VerdictCard[];
  number: number;
}) {
  const scores = cards.flatMap((card) => card.pending ? [] : [card.score]);
  const complete = cards.length > 0 && scores.length === cards.length;
  const low = scores.length ? letterGrade(Math.min(...scores)) : null;
  const high = scores.length ? letterGrade(Math.max(...scores)) : null;
  const spread = low === null ? "—" : low === high ? low : `${low}–${high}`;
  const overall = medianScore(scores);

  return (
    <header className="resume-overview">
      <div className="overview-title">
        <span className="overview-icon" aria-hidden="true"><FileText /></span>
        <div>
          <p className="eyebrow">Resume analysis · v{number}</p>
          <ResumeName key={resumeId} id={resumeId} title={title} />
          <p className="set-filename">File: {filename}</p>
          <p className="lede">Review ATS scores and parsing results for this version.</p>
        </div>
      </div>
      <div className="overview-stats" aria-label="Analysis overview">
        <div className="overall-rating" aria-label="Overall rating">
          <span className="stat-label">Overall grade{complete ? "" : " · Provisional"}</span>
          <div className="overall-rating-value"><strong>{overall === null ? "—" : letterGrade(overall)}</strong>{overall !== null ? <ScoreStamp value={overall} /> : null}</div>
          <span className="overall-rating-note">Median of completed scores</span>
        </div>
        <div><span className="stat-label"><ScanLine aria-hidden="true" /> Systems analyzed</span><strong>{scores.length}<small> / {cards.length}</small></strong></div>
        <div className="grade-range"><span className="stat-label"><Sparkles aria-hidden="true" /> Grade range</span><strong>{spread}</strong></div>
        <p className={`analysis-state ${complete ? "is-complete" : ""}`} role="status">
          {complete ? <CheckCheck aria-hidden="true" /> : <ScanLine aria-hidden="true" />}
          {complete ? "Analysis complete" : "Analysis in progress"}
        </p>
      </div>
    </header>
  );
}
