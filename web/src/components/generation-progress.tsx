import { Check, LoaderCircle, Sparkles } from "lucide-react";

import { stageLabel, type LiveRun } from "~/lib/generation";
import { cn } from "~/lib/utils";

/**
 * Claude-style progress: short status lines that appear as the model reaches
 * each stage, a spinner on the current one and a check on the finished ones.
 */
export function GenerationProgress({
  run,
  compact = false,
}: {
  run?: LiveRun;
  compact?: boolean;
}) {
  if (!run || (run.stages.length === 0 && !run.done && !run.error)) return null;
  const stages = run.stages.length > 0 ? run.stages : ["queued"];
  const heading = run.error
    ? "Couldn't finish this one"
    : run.done
      ? "Finished thinking"
      : "Thinking";

  return (
    <div className={cn("generation-progress", compact && "is-compact")} aria-live="polite">
      <p className="generation-progress-head">
        <span className={cn("thinking-orbit", run.done && "is-done")}>
          <Sparkles size={compact ? 13 : 15} aria-hidden="true" />
        </span>
        {heading}
      </p>
      <ol className="generation-stages">
        {stages.map((label, index) => {
          const active = index === stages.length - 1 && !run.done && !run.error;
          return (
            <li
              key={`${label}-${index}`}
              className={cn("generation-stage", active && "is-active", !active && "is-done")}
            >
              <span className="generation-stage-mark">
                {active ? (
                  <LoaderCircle size={14} className="busy-icon" aria-hidden="true" />
                ) : (
                  <Check size={14} aria-hidden="true" />
                )}
              </span>
              <span>{stageLabel(label)}</span>
            </li>
          );
        })}
        {run.error ? <li className="generation-stage is-error">{run.error}</li> : null}
      </ol>
    </div>
  );
}
