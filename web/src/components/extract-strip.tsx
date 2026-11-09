import type { ExtractField } from "~/server/resumes";
import { ScanText } from "lucide-react";

export function ExtractStrip({ fields }: { fields: ExtractField[] }) {
  return (
    <section className="card extract" aria-label="Résumé details">
      <h2 className="label flex items-center gap-2"><ScanText size={20} aria-hidden="true" /> Résumé details</h2>
      <p className="note">Extracted from your file. Each ATS system analyzes the original separately.</p>
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
