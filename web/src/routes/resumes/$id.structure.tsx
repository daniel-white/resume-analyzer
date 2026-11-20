import { createFileRoute } from "@tanstack/react-router";

import { validateResumeSearch } from "./$id";

import { InlineIdeasCta } from "~/components/ideas-cta";
import { ResumeStructure } from "~/components/resume-structure";
import { useResumeDetail } from "~/components/resume-detail-context";

export const Route = createFileRoute("/resumes/$id/structure")({
  validateSearch: validateResumeSearch,
  component: StructurePane,
});

function StructurePane() {
  const { view, resumeId, versionId, isResume } = useResumeDetail();
  return (
    <div className="resume-detail-pane">
      <ResumeStructure
        key={versionId}
        sections={view.sections}
        fields={view.extract}
        resumeId={resumeId}
        versionId={versionId}
      />
      {isResume ? <InlineIdeasCta view={view} about="the sections we found" /> : null}
    </div>
  );
}
