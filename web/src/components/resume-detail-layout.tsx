import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { AlertTriangle, ArrowLeft, CheckCheck, FileText, ScanLine } from "lucide-react";

import { ideasSubtitle } from "./ideas-cta";
import { Page } from "./page";
import { ResumeName } from "./resume-name";
import {
  ResumeDetailFeedback,
  useResumeDetail,
} from "./resume-detail-context";
import { ResumeDetailTabs } from "./resume-detail-tabs";
import { ResumeViewer } from "./resume-viewer";
import { SiteHeader } from "./site-header";
import { VersionActions } from "./version-actions";

/**
 * The frame for résumé detail routes: the app's Page + SiteHeader template,
 * then either the analysis tabs and pinned PDF rail, or the focused
 * Improvement ideas heading and checklist.
 */
export function ResumeDetailLayout() {
  const { view, resumeId, versionNumber, isResume } = useResumeDetail();
  const isIdeas = useRouterState({
    select: (state) => state.matches.some((match) => match.routeId === "/resumes/$id/ideas"),
  });
  const complete = view.cards.length > 0 && view.cards.every((card) => !card.pending);
  const fileKind = view.file.mediaType === "application/pdf"
    ? "PDF"
    : view.file.mediaType === "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
      ? "DOCX"
      : "file";

  return (
    <Page wide>
      <SiteHeader email={view.email} admin={view.admin} />
      {isIdeas ? (
        <div className="ideas-page">
          <header className="ideas-header">
            <Link
              to="/resumes/$id"
              params={{ id: resumeId }}
              search={(previous) => previous}
              className="ideas-back"
            >
              <ArrowLeft size={15} aria-hidden="true" /> Back to analysis
            </Link>
            <div className="ideas-heading">
              <p className="eyebrow">Improvement ideas · v{versionNumber}</p>
              <ResumeName key={resumeId} id={resumeId} title={view.title} />
              <p className="ideas-subtitle">{ideasSubtitle(view)}</p>
            </div>
            <ResumeDetailFeedback />
          </header>
          <Outlet />
        </div>
      ) : (
        <>
          <header className="resume-detail-header">
            <div className="resume-detail-heading">
              <p className="eyebrow">Résumé analysis · v{versionNumber}</p>
              <ResumeName key={resumeId} id={resumeId} title={view.title} />
              <p className="resume-detail-file"><FileText size={13} aria-hidden="true" /> File: {view.filename}</p>
            </div>
            <div className="resume-detail-header-side">
              <VersionActions />
              <p className={`resume-detail-status ${complete ? "is-complete" : ""}`} role="status">
                {isResume
                  ? complete
                    ? <><CheckCheck size={14} aria-hidden="true" /> Analysis complete</>
                    : <><ScanLine size={14} aria-hidden="true" /> Analysis in progress</>
                  : <><AlertTriangle size={14} aria-hidden="true" /> This file does not appear to be a résumé.</>}
              </p>
              <ResumeDetailFeedback />
            </div>
          </header>
          <ResumeDetailTabs resumeId={resumeId} />
          <div className="resume-detail-layout">
            <div className="resume-detail-main"><Outlet /></div>
            <aside className="resume-detail-rail" id="resume-preview" aria-label="Original document rail">
              <ResumeViewer key={view.file.url} file={view.file} filename={view.filename} />
            </aside>
          </div>
          <div className="mobile-preview-bar">
            <span><FileText size={15} aria-hidden="true" /> Original {fileKind}</span>
            <a href="#resume-preview">View original <ScanLine size={14} aria-hidden="true" /></a>
          </div>
        </>
      )}
    </Page>
  );
}
