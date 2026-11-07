import type { ExtractField } from "~/server/resumes";

export function ExtractStrip({ fields }: { fields: ExtractField[] }) {
  return (
    <section className="card extract" aria-label="What we read">
      <p className="label">What we read</p>
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
