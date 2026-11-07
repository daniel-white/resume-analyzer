import { createFileRoute, notFound } from "@tanstack/react-router";

import { BackLink } from "~/components/back-link";
import { ExtractStrip } from "~/components/extract-strip";
import { FileHeading } from "~/components/file-heading";
import { Lede } from "~/components/lede";
import { NotFound } from "~/components/not-found";
import { Page } from "~/components/page";
import { LiveAnalysis } from "~/components/live-analysis";
import { ResumeViewer } from "~/components/resume-viewer";
import { SiteHeader } from "~/components/site-header";
import { VerdictGrid } from "~/components/verdict-grid";
import { ResumeVersions } from "~/components/resume-versions";
import { getTeardown } from "~/fns/resumes.functions";

export const Route = createFileRoute("/resumes/$id")({
  validateSearch: (search: Record<string, unknown>): { version?: string } => ({
    version: typeof search.version === "string" && search.version ? search.version : undefined,
  }),
  loaderDeps: ({ search }) => ({ versionId: search.version }),
  loader: async ({ params, deps }) => {
    const view = await getTeardown({ data: { id: params.id, versionId: deps.versionId } });
    if (view.kind === "missing") throw notFound();
    return view;
  },
  component: TeardownPage,
  notFoundComponent: NotFound,
});

function TeardownPage() {
  const view = Route.useLoaderData();
  const { id } = Route.useParams();

  return (
    <Page wide>
      <SiteHeader email={view.email} />
      <BackLink />
      <ResumeVersions key={view.resumeId} resumeId={view.resumeId} versionId={view.versionId} versions={view.versions} />
      <LiveAnalysis key={view.versionId} resumeId={id} active={view.kind === "reading"} />
      <FileHeading filename={view.filename} />
      <Lede>{view.lede}</Lede>
      {view.extract ? <ExtractStrip fields={view.extract} /> : null}
      <div className="desk-split mt-8">
        <div className="desk-resume">
          <ResumeViewer key={view.file.url} file={view.file} filename={view.filename} />
        </div>
        <VerdictGrid cards={view.cards} />
      </div>
    </Page>
  );
}
