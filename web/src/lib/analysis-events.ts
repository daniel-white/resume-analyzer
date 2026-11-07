const analysisEvents = ["ResumeUploaded", "ResumeParsed", "AtsRunRequested", "AtsCallbackReceived"];

// Events are live hints, not snapshots. Always reload the authoritative API
// state on open (including reconnect), and serialize bursts of invalidations.
export function watchAnalysis(source: EventSource, refresh: () => Promise<unknown>) {
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

  source.addEventListener("open", reload);
  // Also reconcile on disconnect: a revoked session or a missing resume
  // should reach the route's sign-in/not-found handling, not stay pending.
  source.addEventListener("error", reload);
  for (const name of analysisEvents) source.addEventListener(name, reload);

  return () => {
    stopped = true;
    source.removeEventListener("open", reload);
    source.removeEventListener("error", reload);
    for (const name of analysisEvents) source.removeEventListener(name, reload);
    source.close();
  };
}
