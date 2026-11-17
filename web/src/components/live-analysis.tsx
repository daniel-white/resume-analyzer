import { useRouter } from "@tanstack/react-router";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { watchAnalysis } from "~/lib/analysis-events";
import {
  applyFrame,
  liveKey,
  type GenerationFrame,
  type LiveRun,
} from "~/lib/generation";

type StreamState = Record<string, LiveRun>;

const StreamContext = createContext<StreamState>({});

/** Read the live run for one summary or one diagnosis. Safe outside the
 * provider: components render their stored value as usual. */
export function useLiveRun(key: string): LiveRun | undefined {
  return useContext(StreamContext)[key];
}

const CLEAR_AFTER_MS = 2400;

export function LiveAnalysis({
  resumeId,
  active,
  children,
}: {
  resumeId: string;
  active: boolean;
  children?: ReactNode;
}) {
  const router = useRouter();
  const [runs, setRuns] = useState<StreamState>({});
  const timers = useRef<number[]>([]);

  const onFrame = useCallback((frame: GenerationFrame) => {
    const key = liveKey(frame.flow, frame.version_id, frame.section_id);
    setRuns((current) => ({ ...current, [key]: applyFrame(current[key], frame) }));
    const finished =
      frame.event.type === "done" ||
      frame.event.type === "error" ||
      (frame.event.type === "stage" && frame.event.label === "complete");
    if (!finished) return;
    // Keep the streamed text on screen until the refreshed route can replace
    // it with the persisted result, then drop the live overlay.
    const timer = window.setTimeout(() => {
      setRuns((current) => {
        const next = { ...current };
        delete next[key];
        return next;
      });
    }, CLEAR_AFTER_MS);
    timers.current.push(timer);
  }, []);

  useEffect(() => {
    if (!active) return;
    const source = new EventSource(`/api/resumes/${encodeURIComponent(resumeId)}/events`);
    return watchAnalysis(source, {
      refresh: () => router.invalidate(),
      onFrame,
    });
  }, [resumeId, active, router, onFrame]);

  useEffect(() => {
    const pending = timers.current;
    return () => pending.forEach((timer) => window.clearTimeout(timer));
  }, []);

  const value = useMemo(() => runs, [runs]);
  return <StreamContext.Provider value={value}>{children}</StreamContext.Provider>;
}
