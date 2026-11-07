import { describe, expect, it } from "vitest";

import { summarize, unreachable } from "./health";

describe("summarize", () => {
  it("names each dependency", () => {
    expect(
      summarize({
        status: "ok",
        checks: { postgres: "ok", s3: "ok", smtp: "ok" },
      }),
    ).toBe("ok: postgres ok, s3 ok, smtp ok");
  });

  it("keeps a degraded report readable", () => {
    expect(summarize(unreachable)).toBe(
      "error: postgres error, s3 error, smtp error",
    );
  });
});
