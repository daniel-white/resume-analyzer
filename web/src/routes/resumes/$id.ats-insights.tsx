import { createFileRoute } from "@tanstack/react-router";

import { validateResumeSearch } from "./$id";

import { InlineIdeasCta } from "~/components/ideas-cta";
import { ResumeComparison } from "~/components/resume-comparison";
import { letterGrade, medianScore, ScoreStamp } from "~/components/score";
import { useResumeDetail } from "~/components/resume-detail-context";
import { VerdictGrid } from "~/components/verdict-grid";

export const Route = createFileRoute("/resumes/$id/ats-insights")({
  validateSearch: validateResumeSearch,
  component: AtsInsightsPane,
});

function AtsInsightsPane() {
  const { view, versionId, provider, isResume } = useResumeDetail();
  const scores = view.cards.flatMap((card) => card.pending ? [] : [card.score]);
  const overall = medianScore(scores);
  const low = scores.length ? letterGrade(Math.min(...scores)) : null;
  const high = scores.length ? letterGrade(Math.max(...scores)) : null;
  const spread = low === null ? "—" : low === high ? low : `${low}–${high}`;

  return (
    <div className="resume-detail-pane analysis-results">
      <div className="results-heading">
        <h2>ATS insights</h2>
      </div>
      <section className="insight-grades" aria-label="Grade summary">
        <div>
          <span className="stat-label">Overall grade</span>
          <strong aria-label="Overall rating">{overall === null ? "—" : letterGrade(overall)}</strong>
          {overall === null ? null : <ScoreStamp value={overall} />}
        </div>
        <div>
          <span className="stat-label">Grade range</span>
          <strong>{spread}</strong>
        </div>
        <div>
          <span className="stat-label">Systems analyzed</span>
          <strong>{scores.length}<small> / {view.cards.length}</small></strong>
        </div>
      </section>
      {isResume ? (
        <ResumeComparison key={`comparison-${view.resumeId}`} versions={view.comparisonVersions} versionId={versionId} />
      ) : null}
      <VerdictGrid cards={view.cards} provider={provider} />
      <p className="results-footnote">Simulated ATS results. Scores do not predict hiring outcomes.</p>
      {isResume ? <InlineIdeasCta view={view} about="these ATS results" /> : null}
    </div>
  );
}
