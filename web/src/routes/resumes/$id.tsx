import { createFileRoute, notFound } from "@tanstack/react-router";

import { ResumeDetailsSkeleton } from "~/components/resume-loading";
import { NotFound } from "~/components/not-found";
import { LiveAnalysis } from "~/components/live-analysis";
import {
  IS_NOT_RESUME_PREFIX,
  ResumeDetailProvider,
} from "~/components/resume-detail-context";
import { ResumeDetailLayout } from "~/components/resume-detail-layout";
import { getTeardown } from "~/fns/resumes.functions";

export type ResumeDetailSearch = {
  version?: string | undefined;
  provider?: string | undefined;
};

export function validateResumeSearch(search: Record<string, unknown>): ResumeDetailSearch {
  return {
    version: typeof search["version"] === "string" && search["version"] ? search["version"] : undefined,
    provider: typeof search["provider"] === "string" && search["provider"] ? search["provider"] : undefined,
  };
}

export const Route = createFileRoute("/resumes/$id")({
  validateSearch: validateResumeSearch,
  loaderDeps: ({ search }) => ({ versionId: search.version, provider: search.provider }),
  loader: async ({ params, deps }) => {
    const view = await getTeardown({ data: { id: params.id, versionId: deps.versionId, provider: deps.provider } });
    if (view.kind === "missing") throw notFound();
    return view;
  },
  component: ResumeDetailPage,
  pendingComponent: ResumeDetailsSkeleton,
  pendingMs: 200,
  pendingMinMs: 300,
  notFoundComponent: NotFound,
});

function ResumeDetailPage() {
  const view = Route.useLoaderData();
  const { provider } = Route.useSearch();
  const isResume = !(view.profileSummary?.text?.startsWith(IS_NOT_RESUME_PREFIX) ?? false);

  return (
    <ResumeDetailProvider view={view} provider={provider}>
      <LiveAnalysis
        key={view.versionId}
        resumeId={view.resumeId}
        active={view.comparisonVersions.some((version) => version.status !== "complete")
          || view.kind === "reading"
          || (view.skills === null && isResume)}
      >
        <ResumeDetailLayout />
      </LiveAnalysis>
    </ResumeDetailProvider>
  );
}
