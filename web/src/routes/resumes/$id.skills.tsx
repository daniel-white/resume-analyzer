import { createFileRoute } from "@tanstack/react-router";

import { validateResumeSearch } from "./$id";

import { InlineIdeasCta } from "~/components/ideas-cta";
import { SkillCloud } from "~/components/skill-cloud";
import { useResumeDetail } from "~/components/resume-detail-context";

export const Route = createFileRoute("/resumes/$id/skills")({
  validateSearch: validateResumeSearch,
  component: SkillsPane,
});

function SkillsPane() {
  const { view, versionId, isResume } = useResumeDetail();
  return (
    <div className="resume-detail-pane">
      <SkillCloud
        key={`skills-${versionId}`}
        skills={view.skills}
        sections={view.sections}
        versionId={versionId}
        pending={view.kind === "reading"}
      />
      {isResume ? <InlineIdeasCta view={view} about="the skills we found" /> : null}
    </div>
  );
}
