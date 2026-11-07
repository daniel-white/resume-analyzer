import { createFileRoute, notFound } from "@tanstack/react-router";

import { BackLink } from "~/components/back-link";
import { ExtractStrip } from "~/components/extract-strip";
import { FileHeading } from "~/components/file-heading";
import { Lede } from "~/components/lede";
import { NotFound } from "~/components/not-found";
import { Page } from "~/components/page";
import { RefreshWhileReading } from "~/components/refresh-while-reading";
import { ResumeViewer } from "~/components/resume-viewer";
import { SiteHeader } from "~/components/site-header";
import { VerdictGrid } from "~/components/verdict-grid";
import { getTeardown } from "~/fns/resumes.functions";

export const Route = createFileRoute("/resumes/$id")({
  loader: async ({ params }) => {
    const view = await getTeardown({ data: params.id });
    if (view.kind === "missing") throw notFound();
    return view;
  },
  component: TeardownPage,
  notFoundComponent: NotFound,
});

function TeardownPage() {
  const view = Route.useLoaderData();

  return (
    <Page wide>
      <SiteHeader email={view.email} />
      <BackLink />
      <RefreshWhileReading active={view.kind === "reading"} />
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
