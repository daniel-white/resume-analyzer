import { LoaderCircle } from "lucide-react";

export function DocumentSkeleton({ label = "Loading preview…" }: { label?: string }) {
  return (
    <div className="document-loading" role="status">
      <p className="loading-caption"><LoaderCircle className="busy-icon" size={16} aria-hidden="true" />{label}</p>
      <div className="skeleton-document" aria-hidden="true">
        <div className="loading-shimmer skeleton-name" />
        <div className="loading-shimmer skeleton-contact" />
        {[0, 1, 2].map((section) => (
          <div className="skeleton-section" key={section}>
            <div className="loading-shimmer skeleton-heading" />
            {[0, 1, 2, 3].map((line) => <div className="loading-shimmer skeleton-line" key={line} />)}
          </div>
        ))}
      </div>
    </div>
  );
}

export function ResumeDetailsSkeleton() {
  return (
    <main className="wrap wide details-loading" aria-busy="true" aria-label="Loading resume details">
      <p className="loading-caption" role="status"><LoaderCircle className="busy-icon" size={18} aria-hidden="true" />Opening your resume…</p>
      <div aria-hidden="true">
        <div className="loading-shimmer skeleton-page-title" />
        <div className="loading-shimmer skeleton-page-lede" />
        <div className="sheet-face skeleton-toolbar"><div className="loading-shimmer" /><div className="loading-shimmer" /></div>
        <div className="desk-split">
          <div className="resume-viewer"><DocumentSkeleton /></div>
          <div className="skeleton-results">
            {[0, 1, 2].map((card) => <div className="sheet-face skeleton-card" key={card}><div className="loading-shimmer" /><div className="loading-shimmer" /></div>)}
          </div>
        </div>
      </div>
    </main>
  );
}
