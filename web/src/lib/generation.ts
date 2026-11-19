/**
 * The browser half of the NDJSON streaming protocol. The backend wraps each
 * parsed model line in a GenerationStream SSE frame; this module turns those
 * frames into the small piece of UI state each card needs.
 */

import { isPresentableConfidence } from "./skill-confidence";

export type ModelEvent =
  | { type: "stage"; label: string }
  | { type: "delta"; field: string; value: string }
  | { type: "field"; name: string; value: unknown }
  | { type: "done" }
  | { type: "error"; message: string };

export type GenerationFlow = "summary" | "feedback" | "skills";

/** One skill as it streams in. The validated score arrives with the stored map. */
export type LiveSkill = {
  key: string;
  name: string;
  category: string;
  nature: string;
  prominence: number;
  confidence: number;
};

export type GenerationFrame = {
  resume_id: string;
  version_id: string;
  flow: GenerationFlow;
  section_id: string | null;
  seq: number;
  event: ModelEvent;
};

export type LiveRun = {
  key: string;
  stages: string[];
  text: string;
  skills: LiveSkill[];
  done: boolean;
  error: string | null;
  seq: number;
  updatedAt: number;
};

/** Frames arrive inside the lifecycle EventRecord, not as bare model lines. */
export function parseFrame(data: string): GenerationFrame | null {
  try {
    const record = JSON.parse(data) as { name?: string; payload?: GenerationFrame };
    const frame = record.payload;
    if (
      record.name !== "GenerationStream" ||
      !frame ||
      typeof frame.seq !== "number" ||
      !frame.event ||
      typeof frame.event.type !== "string"
    ) {
      return null;
    }
    return frame;
  } catch {
    return null;
  }
}

export function liveKey(
  flow: GenerationFlow,
  versionId: string,
  sectionId?: string | null,
): string {
  return sectionId ? `${flow}:${versionId}:${sectionId}` : `${flow}:${versionId}`;
}

export function seedRun(key: string): LiveRun {
  return {
    key,
    stages: ["queued"],
    text: "",
    skills: [],
    done: false,
    error: null,
    seq: 0,
    updatedAt: Date.now(),
  };
}

/** Fold one frame into a run. Older/duplicate frames are ignored. */
export function applyFrame(run: LiveRun | undefined, frame: GenerationFrame): LiveRun {
  const key = liveKey(frame.flow, frame.version_id, frame.section_id);
  const current = run ?? seedRun(key);
  if (frame.seq <= current.seq) return current;

  const next: LiveRun = {
    ...current,
    key,
    seq: frame.seq,
    updatedAt: Date.now(),
  };
  const event = frame.event;
  switch (event.type) {
    case "stage": {
      if (event.label === "complete") {
        next.done = true;
      } else if (!next.stages.includes(event.label)) {
        next.stages = [...next.stages, event.label];
      }
      break;
    }
    case "delta":
      next.text += event.value;
      break;
    case "done":
      next.done = true;
      break;
    case "error":
      next.error = event.message || "Generation failed.";
      next.done = true;
      break;
    case "field": {
      // The skill model sends one object per tag, so each frame is one tag.
      if (frame.flow === "skills" && event.name === "skill") {
        const skill = liveSkill(event.value);
        if (skill && !next.skills.some((item) => item.key === skill.key)) {
          next.skills = [...next.skills, skill].slice(0, 40);
        }
        break;
      }
      // Small models sometimes put the whole prose in a `field` instead of
      // `delta`. Accept the string form for the flow's text field so the live
      // view and the final result agree.
      if (typeof event.value !== "string") break;
      const textField = frame.flow === "summary" ? "profile_text" : "text";
      if (event.name === textField) next.text += event.value;
      break;
    }
  }
  return next;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function numberIn(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

/** Validate a streamed skill object before it becomes a live tag. */
function liveSkill(value: unknown): LiveSkill | null {
  if (!isRecord(value) || typeof value["name"] !== "string") return null;
  const name = value["name"].trim();
  if (!name || name.length > 60) return null;
  const key = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  if (!key) return null;
  const confidence = numberIn(value["confidence"], 0, 100, 50);
  if (!isPresentableConfidence(confidence)) return null;
  return {
    key,
    name,
    category: typeof value["category"] === "string" ? value["category"] : "tool",
    nature: typeof value["nature"] === "string" ? value["nature"] : "",
    prominence: numberIn(value["prominence"], 1, 5, 3),
    confidence,
  };
}

const STAGE_LABELS: Record<string, string> = {
  queued: "Getting ready",
  reading: "Reading the document",
  mapping: "Finding your skills",
  understanding: "Finding the through-line",
  comparing: "Comparing against the original",
  diagnosing: "Working out what went wrong",
  writing: "Writing",
  complete: "All done",
};

export function stageLabel(label: string): string {
  return STAGE_LABELS[label] ?? label.replace(/_/g, " ");
}
