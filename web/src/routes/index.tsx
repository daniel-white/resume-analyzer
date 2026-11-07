import { createFileRoute } from "@tanstack/react-router";

import { DropZone } from "~/components/drop-zone";
import { EmptyNote } from "~/components/empty-note";
import { Page } from "~/components/page";
import { LiveAnalysis } from "~/components/live-analysis";
import { ResumeRow } from "~/components/resume-row";
import { SiteHeader } from "~/components/site-header";
import { getLibrary } from "~/fns/resumes.functions";

export const Route = createFileRoute("/")({
  loader: () => getLibrary(),
  component: LibraryPage,
});

function LibraryPage() {
  const { rows, drop, email } = Route.useLoaderData();

  return (
    <Page>
      <SiteHeader email={email} />
      <h1 className="mt-8 text-[clamp(1.75rem,3vw,2.4rem)] font-medium">Your resumes</h1>
      {rows.filter((row) => row.status !== "scored").map((row) => (
        <LiveAnalysis key={row.id} resumeId={row.id} active />
      ))}
      <DropZone copy={drop} />
      {rows.length === 0 ? <EmptyNote /> : null}
      <div className="files">
        {rows.map((row) => (
          <ResumeRow key={row.id} row={row} />
        ))}
      </div>
    </Page>
  );
}
