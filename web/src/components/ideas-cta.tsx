import { Link } from "@tanstack/react-router";
import { ArrowRight, CheckSquare2 } from "lucide-react";

import type { SignedInTeardown } from "~/fns/resumes.functions";

export function ideaCounts(view: SignedInTeardown) {
  return {
    total: view.checklist.length,
    done: view.checklist.filter((item) => item.done).length,
  };
}

export function ideasSubtitle(view: SignedInTeardown): string {
  const { total, done } = ideaCounts(view);
  return `Ideas only · ${done} of ${total} done · your PDF is unchanged`;
}

function IdeasLink({ resumeId, children, className }: {
  resumeId: string;
  children: string;
  className: string;
}) {
  return (
    <Link
      to="/resumes/$id/ideas"
      params={{ id: resumeId }}
      search={(previous) => previous}
      className={className}
    >
      {children} <ArrowRight size={15} aria-hidden="true" />
    </Link>
  );
}

/** Overview-sized call to action into the separate Improvement ideas page. */
export function IdeasCta({ view }: { view: SignedInTeardown }) {
  const { total, done } = ideaCounts(view);
  return (
    <section className="resume-detail-cta" aria-label="Improvement ideas">
      <span className="resume-detail-cta-icon"><CheckSquare2 size={19} aria-hidden="true" /></span>
      <div>
        <h2>{total === 0 ? "Ideas are on their way" : "We found things to improve"}</h2>
        <p>
          {total === 0
            ? "Suggestions for this version will appear as the analysis finishes."
            : `${total} ${total === 1 ? "idea" : "ideas"} for this version${done ? `, ${done} already done` : ""}. Your résumé stays as it is.`}
        </p>
      </div>
      <IdeasLink resumeId={view.resumeId} className="resume-detail-cta-link">
        {total === 0 ? "See ideas" : "See improvement ideas"}
      </IdeasLink>
    </section>
  );
}

/** Smaller, contextual invitation at the foot of a finding pane. */
export function InlineIdeasCta({ view, about }: { view: SignedInTeardown; about: string }) {
  const { total } = ideaCounts(view);
  return (
    <section className="resume-detail-cta is-inline" aria-label="Improve this">
      <span className="resume-detail-cta-icon"><CheckSquare2 size={17} aria-hidden="true" /></span>
      <div>
        <h2>Want to improve this?</h2>
        <p>
          {total === 0
            ? `Ideas about ${about} will appear when the analysis finishes.`
            : `See the ${total} ${total === 1 ? "idea" : "ideas"} about ${about}. Nothing here edits your PDF.`}
        </p>
      </div>
      <IdeasLink resumeId={view.resumeId} className="resume-detail-cta-link">See ideas</IdeasLink>
    </section>
  );
}
