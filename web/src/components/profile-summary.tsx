import { UserRound } from "lucide-react";
import { useEffect, useState } from "react";
import Markdown from "react-markdown";

import { voteResumeSection } from "~/fns/resumes.functions";
import type { ProfileSummary, SectionVote } from "~/server/resumes";
import { VoteButtons } from "./resume-structure";

const PROFILE_SUMMARY_ID = "profile-summary";
const IS_NOT_RESUME_PREFIX = "This does not look like a résumé.";

export function ProfileSummaryCard({
  summary,
  resumeId,
  versionId,
}: {
  summary: ProfileSummary | null;
  resumeId: string;
  versionId: string;
}) {
  const [current, setCurrent] = useState(summary);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setCurrent(summary);
    setError(null);
  }, [summary]);

  const isNotResume = current != null && current.text.startsWith(IS_NOT_RESUME_PREFIX);

  async function vote(choice: SectionVote) {
    if (!current || busy || isNotResume) return;
    const previous = current.feedback ?? null;
    const next = previous === choice ? null : choice;
    setBusy(true);
    setError(null);
    setCurrent({ ...current, feedback: next });
    try {
      const result = await voteResumeSection({
        data: { resumeId, versionId, sectionId: PROFILE_SUMMARY_ID, vote: next },
      });
      if (!result.ok) {
        setCurrent({ ...current, feedback: previous });
        setError(result.message);
        return;
      }
      setCurrent({ ...current, feedback: result.vote });
    } catch {
      setCurrent({ ...current, feedback: previous });
      setError("Could not save your feedback. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="profile-summary" aria-labelledby="profile-summary-heading">
      <div className="profile-summary-heading">
        <span className="profile-summary-icon"><UserRound size={20} aria-hidden="true" /></span>
        <div>
          <p className="profile-summary-eyebrow">
            {isNotResume ? "Not a résumé" : "Model impression"}
          </p>
          <h2 id="profile-summary-heading">
            {isNotResume ? "This doesn’t look like a résumé" : "Who the model thinks you are"}
          </h2>
        </div>
      </div>
      {current ? (
        <>
          <div className="profile-summary-text">
            <Markdown allowedElements={["strong", "em"]} unwrapDisallowed>
              {current.text}
            </Markdown>
          </div>
          {!isNotResume ? (
            <>
              <p className="profile-summary-model">
                {current.engine === "ollama" ? "Local LLM" : "Extractive fallback"} · {current.model}
                <span>Generated from your résumé; may be inaccurate.</span>
              </p>
              <div className="structure-vote" role="group" aria-label="Is this profile summary accurate?">
                <span>Is this summary accurate?</span>
                <VoteButtons
                  feedback={current.feedback ?? null}
                  busy={busy}
                  onVote={(choice) => void vote(choice)}
                  yesLabel="Yes, this summary is accurate"
                  noLabel="No, this summary is not accurate"
                />
              </div>
            </>
          ) : (
            <p className="profile-summary-model">
              <span>Upload a document with work experience, education, skills, or projects.</span>
            </p>
          )}
        </>
      ) : (
        <p className="profile-summary-pending">
          A candidate profile will appear here after this version is parsed.
        </p>
      )}
      {error ? <p className="structure-error" role="alert">{error}</p> : null}
    </section>
  );
}
