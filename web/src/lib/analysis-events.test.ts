import { describe, expect, it, vi } from "vitest";

import { watchAnalysis } from "./analysis-events";

function stream() {
  const source = new EventTarget();
  const close = vi.fn();
  return { source: Object.assign(source, { close }) as unknown as EventSource, close };
}

describe("live analysis", () => {
  it("reconciles on first connection, every named event, reconnect and disconnect", async () => {
    const { source } = stream();
    const refresh = vi.fn().mockResolvedValue(undefined);
    const stop = watchAnalysis(source, refresh);
    for (const name of ["open", "ResumeUploaded", "ResumeParsed", "AtsRunRequested", "AtsCallbackReceived", "error", "open"]) {
      source.dispatchEvent(new Event(name));
      await Promise.resolve();
    }
    expect(refresh).toHaveBeenCalledTimes(7);
    stop();
  });

  it("coalesces a burst but reloads again for events arriving during a refresh", async () => {
    const { source } = stream();
    let resolve!: () => void;
    const refresh = vi.fn().mockImplementationOnce(() => new Promise<void>((done) => { resolve = done; })).mockResolvedValue(undefined);
    const stop = watchAnalysis(source, refresh);
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
    const stop = watchAnalysis(source, refresh);
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
    const stop = watchAnalysis(source, refresh);
    source.dispatchEvent(new Event("open"));
    await Promise.resolve();
    source.dispatchEvent(new Event("open"));
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
  });
});
