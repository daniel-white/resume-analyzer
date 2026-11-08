import type { ParseInput } from "~/server/resumes";

import { Sheet } from "./sheet";

// Render the uploaded file's actual verification extract, never a fixture.
export function ResumeExtract({ parse }: { parse: ParseInput }) {
  return (
    <Sheet>
      <article className="sheet-face p-6" aria-label="Resume verification extract">
        <header className="border-b border-border pb-4">
          <h2 className="font-serif text-2xl font-normal leading-tight">{parse.profile.name || "Your resume"}</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {[parse.profile.email, parse.profile.phone].filter(Boolean).join(" · ")}
          </p>
        </header>
        {parse.profile.sections.map((section, index) => (
          <section className="mt-5" key={`${section.heading}-${index}`}>
            <h3 className="text-xs uppercase tracking-wider text-muted-foreground">{section.heading}</h3>
            <p className="mt-2 whitespace-pre-wrap text-sm">{section.body}</p>
          </section>
        ))}
        <p className="mt-6 text-xs text-muted-foreground">
          Verification extract, not the original layout. Each system reads your file separately.
        </p>
      </article>
    </Sheet>
  );
}
