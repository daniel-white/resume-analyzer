import { createFileRoute } from "@tanstack/react-router";

import { DropZone } from "~/components/drop-zone";
import { EmptyNote } from "~/components/empty-note";
import { Page } from "~/components/page";
import { LiveAnalysis } from "~/components/live-analysis";
import { ResumeRow } from "~/components/resume-row";
import { SiteHeader } from "~/components/site-header";
import { LandingPage } from "~/components/landing-page";
import { getLibrary } from "~/fns/resumes.functions";

export const Route = createFileRoute("/")({
  head: () => ({ meta: [{ title: "The Other Read — Make your experience count" }, { name: "description", content: "See what simulated hiring software picks up from your resume. Upload a PDF or DOCX before signing in, compare ATS readers, and find practical fixes." }] }),
  loader: () => getLibrary(),
  component: LibraryPage,
});

function LibraryPage() {
  const library = Route.useLoaderData();
  if (!library) return <LandingPage />;
  const { rows, drop, email } = library;

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
