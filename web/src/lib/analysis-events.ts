import { parseFrame, type GenerationFrame } from "./generation";

// Lifecycle events are hints, not snapshots. Always reload the authoritative
// API state on open (including reconnect), and serialize bursts of
// invalidations. Token frames never trigger a reload; they update live state.
const lifecycleEvents = [
  "ResumeUploaded",
  "ResumeParsed",
  "AtsRunRequested",
  "AtsCallbackReceived",
  "GenerateMarkdown",
  "AnalyzeFeedback",
] as const;

export type WatchOptions = {
  refresh: () => Promise<unknown>;
  onFrame?: (frame: GenerationFrame) => void;
};

export function watchAnalysis(source: EventSource, options: WatchOptions) {
  const { refresh, onFrame } = options;
  let stopped = false;
  let refreshing = false;
  let pending = false;

  async function reload() {
    if (stopped) return;
    pending = true;
    if (refreshing) return;
    refreshing = true;
    try {
      while (pending && !stopped) {
        pending = false;
        try {
          await refresh();
        } catch {
          // A later event/reconnect retries; route loaders own error handling.
        }
      }
    } finally {
      refreshing = false;
    }
  }

  function frame(event: MessageEvent<string>) {
    if (stopped || !onFrame) return;
    const parsed = parseFrame(event.data);
    if (parsed) onFrame(parsed);
  }

  source.addEventListener("open", reload);
  // Also reconcile on disconnect: a revoked session or a missing resume
  // should reach the route's sign-in/not-found handling, not stay pending.
  source.addEventListener("error", reload);
  for (const name of lifecycleEvents) source.addEventListener(name, reload);
  source.addEventListener("GenerationStream", frame as EventListener);

  return () => {
    stopped = true;
    source.removeEventListener("open", reload);
    source.removeEventListener("error", reload);
    for (const name of lifecycleEvents) source.removeEventListener(name, reload);
    source.removeEventListener("GenerationStream", frame as EventListener);
    source.close();
  };
}
