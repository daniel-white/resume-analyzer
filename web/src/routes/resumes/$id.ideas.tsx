import { createFileRoute } from "@tanstack/react-router";

import { validateResumeSearch } from "./$id";

import { SecondDraft } from "~/components/second-draft";
import { useResumeDetail } from "~/components/resume-detail-context";

export const Route = createFileRoute("/resumes/$id/ideas")({
  validateSearch: validateResumeSearch,
  component: IdeasPane,
});

function IdeasPane() {
  const { view, versionId } = useResumeDetail();
  return (
    <SecondDraft
      key={`checklist-${versionId}`}
      items={view.checklist}
      resumeId={view.resumeId}
      versionId={versionId}
      pending={view.kind === "reading"}
    />
  );
}
