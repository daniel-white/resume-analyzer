import { describe, expect, it } from "vitest";

import {
  applyFrame,
  liveKey,
  parseFrame,
  seedRun,
  type GenerationFrame,
} from "./generation";

function frame(
  seq: number,
  event: GenerationFrame["event"],
  flow: GenerationFrame["flow"] = "feedback",
  sectionId: string | null = flow === "summary" ? null : "section-1",
): GenerationFrame {
  return {
    resume_id: "resume-1",
    version_id: "version-1",
    flow,
    section_id: sectionId,
    seq,
    event,
  };
}

describe("generation stream", () => {
  it("parses a GenerationStream event record and rejects other messages", () => {
    const value = frame(1, { type: "stage", label: "reading" });
    expect(parseFrame(JSON.stringify({ name: "GenerationStream", payload: value }))).toEqual(value);
    expect(parseFrame(JSON.stringify({ name: "ResumeParsed", payload: value }))).toBeNull();
    expect(parseFrame("not json")).toBeNull();
  });

  it("folds stages and deltas into one live run keyed by flow and section", () => {
    const key = liveKey("feedback", "version-1", "section-1");
    let run = seedRun(key);
    run = applyFrame(run, frame(1, { type: "stage", label: "reading" }));
    run = applyFrame(run, frame(2, { type: "delta", field: "text", value: "The heading " }));
    run = applyFrame(run, frame(3, { type: "delta", field: "text", value: "is wrong." }));

    expect(run.key).toBe(key);
    expect(run.stages).toEqual(["queued", "reading"]);
    expect(run.text).toBe("The heading is wrong.");
  });

  it("ignores stale or duplicate frames and marks completion", () => {
    const key = liveKey("feedback", "version-1", "section-1");
    let run = applyFrame(seedRun(key), frame(2, { type: "delta", field: "text", value: "new" }));
    run = applyFrame(run, frame(1, { type: "delta", field: "text", value: "old" }));
    expect(run.text).toBe("new");

    run = applyFrame(run, frame(3, { type: "stage", label: "complete" }));
    expect(run.done).toBe(true);
    expect(run.seq).toBe(3);
  });

  it("accepts prose delivered in string field events", () => {
    const key = liveKey("summary", "version-1");
    let run = seedRun(key);
    run = applyFrame(
      run,
      frame(1, { type: "field", name: "is_resume", value: true }, "summary"),
    );
    run = applyFrame(
      run,
      frame(2, { type: "field", name: "profile_text", value: "You build " }, "summary"),
    );
    run = applyFrame(
      run,
      frame(3, { type: "field", name: "profile_text", value: "systems." }, "summary"),
    );
    expect(run.text).toBe("You build systems.");
  });

  it("accumulates one live skill per streamed skill field", () => {
    const key = liveKey("skills", "version-1");
    let run = applyFrame(seedRun(key), frame(1, { type: "stage", label: "mapping" }, "skills", null));
    run = applyFrame(run, frame(2, {
      type: "field",
      name: "skill",
      value: {
        name: "Triage", category: "method", nature: "hard",
        prominence: 5, confidence: 90,
      },
    }, "skills", null));
    run = applyFrame(run, frame(3, {
      type: "field",
      name: "skill",
      value: {
        name: "Triage", category: "method", nature: "hard",
        prominence: 5, confidence: 90,
      },
    }, "skills", null));
    run = applyFrame(run, frame(4, {
      type: "field",
      name: "skill",
      value: { name: "Patient education", category: "practice", nature: "people" },
    }, "skills", null));
    run = applyFrame(run, frame(5, { type: "field", name: "skill", value: { name: 42 } }, "skills", null));

    expect(run.skills).toHaveLength(2);
    expect(run.skills[0]).toMatchObject({ key: "triage", nature: "hard", prominence: 5 });
    expect(run.skills[1]).toMatchObject({ key: "patient-education", nature: "people" });
    expect(run.text).toBe("");
  });

  it("surfaces model errors as a finished run", () => {
    const run = applyFrame(seedRun("key"), frame(1, { type: "error", message: "Model stopped." }));
    expect(run.error).toBe("Model stopped.");
    expect(run.done).toBe(true);
  });
});
