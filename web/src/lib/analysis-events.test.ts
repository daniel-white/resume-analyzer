import { describe, expect, it, vi } from "vitest";

import { watchAnalysis } from "./analysis-events";
import type { GenerationFrame } from "./generation";

function stream() {
  const source = new EventTarget();
  const close = vi.fn();
  return { source: Object.assign(source, { close }) as unknown as EventSource, close };
}

const frame: GenerationFrame = {
  resume_id: "resume-1",
  version_id: "version-1",
  flow: "summary",
  section_id: null,
  seq: 1,
  event: { type: "delta", field: "profile_text", value: "You build " },
};

function frameEvent(value: unknown) {
  return Object.assign(new Event("GenerationStream"), {
    data: JSON.stringify(value),
  });
}

describe("live analysis", () => {
  it("reconciles on first connection, every named event, reconnect and disconnect", async () => {
    const { source } = stream();
    const refresh = vi.fn().mockResolvedValue(undefined);
    const stop = watchAnalysis(source, { refresh });
    for (const name of [
      "open",
      "ResumeUploaded",
      "ResumeParsed",
      "AtsRunRequested",
      "AtsCallbackReceived",
      "GenerateMarkdown",
      "AnalyzeFeedback",
      "error",
      "open",
    ]) {
      source.dispatchEvent(new Event(name));
      await Promise.resolve();
    }
    expect(refresh).toHaveBeenCalledTimes(9);
    stop();
  });

  it("coalesces a burst but reloads again for events arriving during a refresh", async () => {
    const { source } = stream();
    let resolve!: () => void;
    const refresh = vi.fn().mockImplementationOnce(() => new Promise<void>((done) => { resolve = done; })).mockResolvedValue(undefined);
    const stop = watchAnalysis(source, { refresh });
    source.dispatchEvent(new Event("open"));
    source.dispatchEvent(new Event("ResumeParsed"));
    source.dispatchEvent(new Event("AtsCallbackReceived"));
    expect(refresh).toHaveBeenCalledTimes(1);
    resolve();
    await Promise.resolve();
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
  });

  it("closes the stream and cancels queued updates on cleanup", async () => {
    const { source, close } = stream();
    let resolve!: () => void;
    const refresh = vi.fn(() => new Promise<void>((done) => { resolve = done; }));
    const stop = watchAnalysis(source, { refresh });
    source.dispatchEvent(new Event("open"));
    source.dispatchEvent(new Event("ResumeParsed"));
    stop();
    resolve();
    await Promise.resolve();
    source.dispatchEvent(new Event("AtsCallbackReceived"));
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledOnce();
  });

  it("can reconcile again after a failed refresh", async () => {
    const { source } = stream();
    const refresh = vi.fn().mockRejectedValueOnce(new Error("offline")).mockResolvedValue(undefined);
    const stop = watchAnalysis(source, { refresh });
    source.dispatchEvent(new Event("open"));
    await Promise.resolve();
    source.dispatchEvent(new Event("open"));
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
  });

  it("forwards parsed token frames without refreshing the route", async () => {
    const { source } = stream();
    const refresh = vi.fn().mockResolvedValue(undefined);
    const onFrame = vi.fn();
    const stop = watchAnalysis(source, { refresh, onFrame });
    source.dispatchEvent(frameEvent({ name: "GenerationStream", payload: frame }));
    source.dispatchEvent(frameEvent({ name: "GenerationStream", payload: { nope: true } }));
    expect(onFrame).toHaveBeenCalledTimes(1);
    expect(onFrame).toHaveBeenCalledWith(frame);
    expect(refresh).not.toHaveBeenCalled();
    stop();
  });
});
