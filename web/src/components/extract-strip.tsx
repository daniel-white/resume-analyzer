import type { ExtractField } from "~/server/resumes";
import { ScanText } from "lucide-react";

export function ExtractStrip({ fields }: { fields: ExtractField[] }) {
  return (
    <section className="card extract" aria-label="What we read">
      <h2 className="label flex items-center gap-2"><ScanText size={20} aria-hidden="true" /> What we read</h2>
      <p className="note">Check this is your resume. The systems below open the file themselves.</p>
      <dl className="facts">
        {fields.map((field) => (
          <div key={field.label}>
            <dt>{field.label}</dt>
            <dd>{field.value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
