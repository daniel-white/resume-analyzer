import { Files, ScanLine, WandSparkles } from "lucide-react";

export function EmptyNote() {
  return (
    <section className="empty-library" aria-label="Getting started">
      <p className="empty-library-note">No résumés yet.</p>
      <h2>How it works</h2>
      <ol className="empty-library-steps">
        <li><span className="empty-step-icon"><ScanLine aria-hidden="true" /></span><div><h3>Analyze your résumé</h3><p>Each simulated ATS system processes your file independently.</p></div></li>
        <li><span className="empty-step-icon"><WandSparkles aria-hidden="true" /></span><div><h3>Review results</h3><p>Check extracted content, missed details, and score factors.</p></div></li>
        <li><span className="empty-step-icon"><Files aria-hidden="true" /></span><div><h3>Upload revisions</h3><p>Keep previous versions and their results in one place.</p></div></li>
      </ol>
      <p className="empty-library-disclaimer">Simulated ATS results. Scores do not predict hiring outcomes.</p>
    </section>
  );
}
