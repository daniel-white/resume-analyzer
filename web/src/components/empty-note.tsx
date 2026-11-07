import { Files, ScanLine, WandSparkles } from "lucide-react";

export function EmptyNote() {
  return (
    <section className="empty-library" aria-label="Getting started">
      <p className="empty-library-note">Nothing stored yet.</p>
      <h2>A little clarity before you apply.</h2>
      <ol className="empty-library-steps">
        <li><span className="empty-step-icon"><ScanLine aria-hidden="true" /></span><div><h3>See the other side</h3><p>Independent simulated ATS systems read your original file, each in its own way.</p></div></li>
        <li><span className="empty-step-icon"><WandSparkles aria-hidden="true" /></span><div><h3>Know what to refine</h3><p>See what was kept, what was missed, and the details behind each score.</p></div></li>
        <li><span className="empty-step-icon"><Files aria-hidden="true" /></span><div><h3>Make room for revisions</h3><p>Add a new version when you’re ready. Previous files and results stay together.</p></div></li>
      </ol>
      <p className="empty-library-disclaimer">A useful second opinion, not a prediction of hiring outcomes.</p>
    </section>
  );
}
