import { useRouter } from "@tanstack/react-router";
import { useEffect } from "react";

import { watchAnalysis } from "~/lib/analysis-events";

export function LiveAnalysis({ resumeId, active }: { resumeId: string; active: boolean }) {
  const router = useRouter();
  useEffect(() => {
    if (!active) return;
    const source = new EventSource(`/api/resumes/${encodeURIComponent(resumeId)}/events`);
    return watchAnalysis(source, () => router.invalidate());
  }, [resumeId, active, router]);
  return null;
}
