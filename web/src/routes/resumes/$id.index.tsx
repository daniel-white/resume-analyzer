import { Link, createFileRoute } from "@tanstack/react-router";

import { validateResumeSearch } from "./$id";
import { FileClock, Layers3, Sparkles, Tags } from "lucide-react";

import { IdeasCta, ideaCounts } from "~/components/ideas-cta";
import { ProfileSummaryCard } from "~/components/profile-summary";
import { letterGrade, medianScore } from "~/components/score";
import { VersionDanger } from "~/components/version-danger";
import { useResumeDetail } from "~/components/resume-detail-context";

export const Route = createFileRoute("/resumes/$id/")({
  validateSearch: validateResumeSearch,
  component: OverviewPane,
});

function OverviewPane() {
  const { view, versionId, resumeId, isResume } = useResumeDetail();
  const scores = view.cards.flatMap((card) => card.pending ? [] : [card.score]);
  const overall = medianScore(scores);
  const low = scores.length ? letterGrade(Math.min(...scores)) : null;
  const high = scores.length ? letterGrade(Math.max(...scores)) : null;
  const spread = low === null ? "—" : low === high ? low : `${low}–${high}`;
  const { total, done } = ideaCounts(view);
  const version = view.comparisonVersions.find((item) => item.id === versionId);
  const summaryPending = version?.status !== "complete";
  const links = [
    { label: "Structure", to: "/resumes/$id/structure" as const, icon: Layers3, meta: view.sections ? `${view.sections.sections.length} sections found` : "Reading your sections" },
    { label: "Skills", to: "/resumes/$id/skills" as const, icon: Tags, meta: view.skills ? `${view.skills.skills.length} skills mapped` : "Reading your skills" },
    { label: "ATS insights", to: "/resumes/$id/ats-insights" as const, icon: Sparkles, meta: spread === "—" ? "Results on the way" : `Grades ${spread} · median ${overall === null ? "—" : letterGrade(overall)}` },
    { label: "Improvement ideas", to: "/resumes/$id/ideas" as const, icon: FileClock, meta: total === 0 ? "Ideas on the way" : `${total} ${total === 1 ? "idea" : "ideas"} · ${done} done` },
  ];

  return (
    <div className="resume-detail-pane">
      <nav className="insight-strip" aria-label="Quick insights">
        {links.map((link) => (
          <Link
            key={link.label}
            to={link.to}
            params={{ id: resumeId }}
            search={(previous) => previous}
            className="insight-signpost"
          >
            <span className="insight-icon"><link.icon size={17} aria-hidden="true" /></span>
            <strong>{link.label}</strong>
            <span className="insight-meta">{link.meta}</span>
          </Link>
        ))}
      </nav>
      <ProfileSummaryCard
        key={`summary-${versionId}`}
        summary={view.profileSummary}
        resumeId={view.resumeId}
        versionId={versionId}
        pending={summaryPending}
      />
      {isResume ? <IdeasCta view={view} /> : null}
      <VersionDanger />
    </div>
  );
}
