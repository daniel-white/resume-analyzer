import { UserRound } from "lucide-react";
import { useEffect, useState } from "react";
import Markdown from "react-markdown";

import { voteResumeSection } from "~/fns/resumes.functions";
import { liveKey, seedRun } from "~/lib/generation";
import type { ProfileSummary, SectionVote } from "~/server/resumes";
import { cn } from "~/lib/utils";
import { GenerationProgress } from "./generation-progress";
import { useLiveRun } from "./live-analysis";
import { VoteButtons } from "./resume-structure";

const PROFILE_SUMMARY_ID = "profile-summary";
const IS_NOT_RESUME_PREFIX = "This does not look like a résumé.";

export function ProfileSummaryCard({
  summary,
  resumeId,
  versionId,
  pending = false,
}: {
  summary: ProfileSummary | null;
  resumeId: string;
  versionId: string;
  pending?: boolean;
}) {
  const [current, setCurrent] = useState(summary);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const key = liveKey("summary", versionId);
  const live = useLiveRun(key);

  useEffect(() => {
    setCurrent(summary);
    setError(null);
  }, [summary]);

  // While the model writes, its tokens replace the stored text. Once the run
  // clears, the refreshed route value takes over with no visible jump.
  const liveText = live?.text ?? "";
  const streaming = Boolean(live && !live.done && !live.error && liveText.trim());
  const text = liveText.trim() ? liveText : (current?.text ?? "");
  const isResume = !text.startsWith(IS_NOT_RESUME_PREFIX);
  const isEmpty = text.trim() === "";
  // Show the thinking state immediately, before the first token frame arrives.
  const progressRun = live ?? (pending && isEmpty && isResume ? seedRun(key) : undefined);

  async function vote(choice: SectionVote) {
    if (!current || busy || !isResume || streaming) return;
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
    <section
      className={cn("profile-summary", streaming && "is-streaming")}
      aria-labelledby="profile-summary-heading"
    >
      <div className="profile-summary-heading">
        <span className="profile-summary-icon"><UserRound size={20} aria-hidden="true" /></span>
        <div>
          <p className="profile-summary-eyebrow">
            {streaming ? "Writing now" : isResume ? "Model impression" : "Not a résumé"}
          </p>
          <h2 id="profile-summary-heading">
            {isResume ? "Who the model thinks you are" : "This doesn’t look like a résumé"}
          </h2>
        </div>
      </div>
      {progressRun ? <GenerationProgress run={progressRun} /> : null}
      {!isEmpty ? (
        <>
          <div
            className={cn("profile-summary-text", streaming && "is-live")}
            aria-live={streaming ? "polite" : undefined}
          >
            <Markdown allowedElements={["strong", "em"]} unwrapDisallowed>
              {text}
            </Markdown>
          </div>
          {isResume && !streaming && current ? (
            <>
              <p className="profile-summary-model">
                Local LLM · {current.model}
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
          ) : null}
          {!isResume && !streaming ? (
            <p className="profile-summary-model">
              <span>Upload a document with work experience, education, skills, or projects.</span>
            </p>
          ) : null}
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
