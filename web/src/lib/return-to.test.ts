import { describe, expect, it } from "vitest";
import { safeReturnTo } from "./return-to";

describe("safeReturnTo", () => {
  const id = "12345678-1234-1234-1234-123456789abc";
  it("preserves the exact version and provider through sign-in", () => {
    const path = `/resumes/${id}?version=${id}&provider=greenline`;
    expect(safeReturnTo(path)).toBe(path);
  });
  it("allows the exact admin page", () => {
    expect(safeReturnTo("/admin")).toBe("/admin");
  });
  it("rejects external URLs, admin subpaths, and unrelated routes", () => {
    for (const path of [
      "https://evil.example",
      "//evil.example",
      "/login",
      "/admin/",
      "/admin?tab=open",
      "/admin/../../login",
      `/resumes/${id}/../../login`,
      `/resumes/${id}\\evil`,
      undefined,
    ]) {
      expect(safeReturnTo(path)).toBeUndefined();
    }
  });
});
