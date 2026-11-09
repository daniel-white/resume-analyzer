import { ScanText, ThumbsDown, ThumbsUp } from "lucide-react";
import { useState } from "react";

import { voteResumeSection } from "~/fns/resumes.functions";
import { providerFound, sectionKindLabel } from "~/lib/sections";
import { cn } from "~/lib/utils";
import type {
  DetectedSection,
  SectionMap,
  SectionVote,
  VerdictCard,
} from "~/server/resumes";

export function ResumeStructure({
  sections,
  cards,
  resumeId,
  versionId,
  onHighlight,
  initialSelectedId = null,
}: {
  sections: SectionMap | null;
  cards: VerdictCard[];
  resumeId: string;
  versionId: string;
  onHighlight: (kind: DetectedSection["kind"] | null) => void;
  initialSelectedId?: string | null;
}) {
  const [map, setMap] = useState<SectionMap | null>(sections);
  const [selectedId, setSelectedId] = useState<string | null>(initialSelectedId);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!map || map.sections.length === 0) {
    return (
      <section className="card structure" aria-label="Our reading of your resume">
        <h2 className="label flex items-center gap-2">
          <ScanText size={20} aria-hidden="true" /> Our read of your resume
        </h2>
        <p className="note">
          {map
            ? "We could not find any sections in this file."
            : "Reading your file into sections. This appears before the first ATS result."}
        </p>
      </section>
    );
  }

  const selected = map.sections.find((section) => section.id === selectedId) ?? null;
  const complete = cards.filter((card) => !card.pending);

  function select(section: DetectedSection) {
    const next = selectedId === section.id ? null : section.id;
    setSelectedId(next);
    onHighlight(next ? section.kind : null);
  }

  async function vote(section: DetectedSection, choice: SectionVote) {
    if (busyId) return;
    const previous = section.feedback ?? null;
    const next = previous === choice ? null : choice;
    setBusyId(section.id);
    setError(null);
    setFeedback(setMap, section.id, next);
    try {
      const result = await voteResumeSection({
        data: { resumeId, versionId, sectionId: section.id, vote: next },
      });
      if (!result.ok) {
        setFeedback(setMap, section.id, previous);
        setError(result.message);
        return;
      }
      setFeedback(setMap, section.id, result.vote);
    } catch {
      setFeedback(setMap, section.id, previous);
      setError("Could not save your feedback. Please try again.");
    } finally {
      setBusyId(null);
    }
  }

  return (
    <section className="card structure" aria-label="Our reading of your resume">
      <h2 className="label flex items-center gap-2">
        <ScanText size={20} aria-hidden="true" /> Our read of your resume
      </h2>
      <p className="note">
        Every section below came from your file. Select one to see which
        systems found it and to mark whether we read it correctly.
      </p>
      <ol className="structure-sections">
        {map.sections.map((section) => {
          const open = section.id === selectedId;
          return (
            <li key={section.id}>
              <button
                type="button"
                className={cn("structure-section", open && "is-selected")}
                aria-expanded={open}
                aria-controls={open ? `structure-body-${section.id}` : undefined}
                onClick={() => select(section)}
              >
                <span className="structure-kind">{sectionKindLabel(section.kind)}</span>
                <span className="structure-heading">{section.heading}</span>
                <span className="structure-confidence" title="Detection confidence">
                  {Math.round(section.confidence * 100)}%
                </span>
              </button>
              {open ? (
                <div className="structure-body" id={`structure-body-${section.id}`}>
                  <p className="whitespace-pre-wrap">
                    {section.body || "No text under this heading."}
                  </p>
                  <div
                    className="structure-vote"
                    role="group"
                    aria-label={`Was ${section.heading} read correctly?`}
                  >
                    <span>Did we read this section correctly?</span>
                    <VoteButtons
                      feedback={section.feedback ?? null}
                      busy={busyId === section.id}
                      onVote={(choice) => void vote(section, choice)}
                    />
                  </div>
                  {error ? <p className="structure-error" role="alert">{error}</p> : null}
                  <div className="structure-compare">
                    <h3>Which systems found it</h3>
                    {complete.length === 0 ? (
                      <p className="structure-pending">No system has finished reading yet.</p>
                    ) : (
                      <ul>
                        {complete.map((card) => {
                          const found = providerFound(section.kind, {
                            parse: card.parse,
                            kept: card.kept,
                          });
                          return (
                            <li key={card.id} className={found ? "is-found" : "is-missed"}>
                              <span>{card.name}</span>
                              <strong>{found ? "Found" : "Missed"}</strong>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                  </div>
                </div>
              ) : null}
            </li>
          );
        })}
      </ol>
    </section>
  );
}

export function VoteButtons({
  feedback,
  busy,
  onVote,
}: {
  feedback: SectionVote | null;
  busy: boolean;
  onVote: (choice: SectionVote) => void;
}) {
  return (
    <>
      <button
        type="button"
        className={cn("vote", feedback === "up" && "is-on")}
        aria-label="Yes, this section was read correctly"
        title="Yes, this section was read correctly"
        aria-pressed={feedback === "up"}
        disabled={busy}
        onClick={() => onVote("up")}
      >
        <ThumbsUp
          size={16}
          aria-hidden="true"
          fill={feedback === "up" ? "currentColor" : "none"}
        />
      </button>
      <button
        type="button"
        className={cn("vote", feedback === "down" && "is-on")}
        aria-label="No, this section was not read correctly"
        title="No, this section was not read correctly"
        aria-pressed={feedback === "down"}
        disabled={busy}
        onClick={() => onVote("down")}
      >
        <ThumbsDown
          size={16}
          aria-hidden="true"
          fill={feedback === "down" ? "currentColor" : "none"}
        />
      </button>
    </>
  );
}

function setFeedback(
  setMap: (update: (current: SectionMap | null) => SectionMap | null) => void,
  sectionId: string,
  vote: SectionVote | null,
) {
  setMap((current) =>
    current
      ? {
          ...current,
          sections: current.sections.map((section) =>
            section.id === sectionId ? { ...section, feedback: vote } : section,
          ),
        }
      : current,
  );
}
