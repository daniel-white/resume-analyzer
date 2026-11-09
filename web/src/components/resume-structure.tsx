import { ScanText, ThumbsDown, ThumbsUp } from "lucide-react";
import { useState, useEffect } from "react";

import { voteResumeSection } from "~/fns/resumes.functions";
import { sectionKindLabel } from "~/lib/sections";
import { cn } from "~/lib/utils";
import type {
  DetectedSection,
  ExtractField,
  SectionMap,
  SectionVote,
} from "~/server/resumes";

export function ResumeStructure({
  sections,
  fields = null,
  resumeId,
  versionId,
  onHighlight,
  initialSelectedId = null,
}: {
  sections: SectionMap | null;
  fields?: ExtractField[] | null;
  resumeId: string;
  versionId: string;
  onHighlight: (kind: DetectedSection["kind"] | null) => void;
  initialSelectedId?: string | null;
}) {
  const [map, setMap] = useState<SectionMap | null>(sections);
  const [selectedId, setSelectedId] = useState<string | null>(initialSelectedId);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Initialize selectedId only once on mount, not on every sections change
  useEffect(() => {
    setMap(sections);
  }, [sections]);

  if (initialSelectedId && !selectedId) {
    setSelectedId(initialSelectedId);
  }

  if (!map || map.sections.length === 0) {
    return (
      <section className="card structure" aria-label="Résumé details">
        <h2 className="label flex items-center gap-2">
          <ScanText size={20} aria-hidden="true" /> Résumé details
        </h2>
        <ResumeFacts fields={fields} />
        <p className="note">
          {map
            ? "We could not find any sections in this file."
            : "Reading your file into sections. This appears before the first ATS result."}
        </p>
      </section>
    );
  }

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
    <section className="card structure" aria-label="Résumé details">
      <h2 className="label flex items-center gap-2">
        <ScanText size={20} aria-hidden="true" /> Résumé details
      </h2>
      <ResumeFacts fields={fields} />
      <p className="note">
        Select a section to check our reading and compare ATS results.
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
                </div>
              ) : null}
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function ResumeFacts({ fields }: { fields: ExtractField[] | null }) {
  if (!fields?.length) return null;
  return (
    <dl className="facts">
      {fields.map((field) => (
        <div key={field.label}><dt>{field.label}</dt><dd>{field.value}</dd></div>
      ))}
    </dl>
  );
}

export function VoteButtons({
  feedback,
  busy,
  onVote,
  yesLabel = "Yes, this section was read correctly",
  noLabel = "No, this section was not read correctly",
}: {
  feedback: SectionVote | null;
  busy: boolean;
  onVote: (choice: SectionVote) => void;
  yesLabel?: string;
  noLabel?: string;
}) {
  return (
    <>
      <button
        type="button"
        className={cn("vote", feedback === "up" && "is-on")}
        aria-label={yesLabel}
        title={yesLabel}
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
        aria-label={noLabel}
        title={noLabel}
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
