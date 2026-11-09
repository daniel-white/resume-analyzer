import { createFileRoute, notFound } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { ResumeOverview } from "~/components/resume-overview";
import { ResumeDetailsSkeleton } from "~/components/resume-loading";
import { NotFound } from "~/components/not-found";
import { Page } from "~/components/page";
import { LiveAnalysis } from "~/components/live-analysis";
import { ResumeViewer } from "~/components/resume-viewer";
import { SiteHeader } from "~/components/site-header";
import { VerdictGrid } from "~/components/verdict-grid";
import { ResumeVersions } from "~/components/resume-versions";
import { ResumeComparison } from "~/components/resume-comparison";
import { ResumeStructure } from "~/components/resume-structure";
import { ProfileSummaryCard } from "~/components/profile-summary";
import { getTeardown } from "~/fns/resumes.functions";
import type { SectionKind } from "~/server/resumes";

const IS_NOT_RESUME_PREFIX = "This does not look like a résumé.";

export const Route = createFileRoute("/resumes/$id")({
  validateSearch: (search: Record<string, unknown>): { version?: string; provider?: string } => ({
    version: typeof search.version === "string" && search.version ? search.version : undefined,
    provider: typeof search.provider === "string" && search.provider ? search.provider : undefined,
  }),
  loaderDeps: ({ search }) => ({ versionId: search.version, provider: search.provider }),
  loader: async ({ params, deps }) => {
    const view = await getTeardown({ data: { id: params.id, versionId: deps.versionId, provider: deps.provider } });
    if (view.kind === "missing") throw notFound();
    return view;
  },
  component: TeardownPage,
  pendingComponent: ResumeDetailsSkeleton,
  pendingMs: 200,
  pendingMinMs: 300,
  notFoundComponent: NotFound,
});

function TeardownPage() {
  const view = Route.useLoaderData();
  const { id } = Route.useParams();
  const { provider } = Route.useSearch();
  const [highlightKind, setHighlightKind] = useState<SectionKind | null>(null);
  const isNotResume = view.profileSummary?.text?.startsWith(IS_NOT_RESUME_PREFIX) ?? false;

  useEffect(() => setHighlightKind(null), [view.versionId]);

  return (
    <Page wide>
      <SiteHeader email={view.email} />
      <ResumeOverview resumeId={view.resumeId} title={view.title} filename={view.filename} cards={view.cards}
        number={view.versions.find((version) => version.id === view.versionId)!.number} isNotResume={isNotResume} />
      <ResumeVersions key={view.resumeId} resumeId={view.resumeId} versionId={view.versionId} versions={view.versions} />
      {isNotResume ? null : (
        <ResumeComparison key={`comparison-${view.resumeId}`} versions={view.comparisonVersions} versionId={view.versionId} />
      )}
      {isNotResume ? null : (
        <LiveAnalysis key={view.versionId} resumeId={id} active={view.comparisonVersions.some((version) => version.status !== "complete")} />
      )}
      <ProfileSummaryCard
        key={`summary-${view.versionId}`}
        summary={view.profileSummary}
        resumeId={view.resumeId}
        versionId={view.versionId}
      />
      {isNotResume ? null : (
        <ResumeStructure
          key={view.versionId}
          fields={view.extract}
          sections={view.sections}
          resumeId={view.resumeId}
          versionId={view.versionId}
          onHighlight={setHighlightKind}
        />
      )}
      <div className="desk-split resume-content">
        <div className="desk-resume">
          <ResumeViewer key={view.file.url} file={view.file} filename={view.filename} />
        </div>
        {isNotResume ? null : (
          <section className="analysis-results" aria-label="System results">
            <div className="results-heading"><h2>ATS results</h2></div>
            <VerdictGrid cards={view.cards} provider={provider} highlightKind={highlightKind} />
            <p className="results-footnote">Simulated ATS results. Scores do not predict hiring outcomes.</p>
          </section>
        )}
      </div>
    </Page>
  );
}
