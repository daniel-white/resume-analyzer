import { Link } from "@tanstack/react-router";

const tabs = [
  { label: "Overview", to: "/resumes/$id", exact: true },
  { label: "Structure", to: "/resumes/$id/structure", exact: false },
  { label: "Skills", to: "/resumes/$id/skills", exact: false },
  { label: "ATS insights", to: "/resumes/$id/ats-insights", exact: false },
] as const;

/** One flat bar; every tab is a real, reloadable route. */
export function ResumeDetailTabs({ resumeId }: { resumeId: string }) {
  return (
    <nav className="resume-detail-tabs" aria-label="Analysis sections">
      {tabs.map((tab) => (
        <Link
          key={tab.label}
          to={tab.to}
          params={{ id: resumeId }}
          search={(previous) => previous}
          activeOptions={{ exact: tab.exact }}
          activeProps={{ className: "resume-detail-tab is-active", "aria-current": "page" }}
          inactiveProps={{ className: "resume-detail-tab" }}
        >
          {tab.label}
        </Link>
      ))}
    </nav>
  );
}
