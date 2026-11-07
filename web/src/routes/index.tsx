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
      <div className="library-heading">
        <h1>Your resumes</h1>
        <p>{rows.length === 0 ? "Upload a resume to view ATS scores and parsing results." : "Manage your resumes, versions, and ATS results."}</p>
      </div>
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
