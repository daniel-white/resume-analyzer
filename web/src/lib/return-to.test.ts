import { describe, expect, it } from "vitest";
import { resumeReturnTo } from "./return-to";

describe("resumeReturnTo", () => {
  const id = "12345678-1234-1234-1234-123456789abc";
  it("preserves the exact version and provider through sign-in", () => {
    const path = `/resumes/${id}?version=${id}&provider=greenline`;
    expect(resumeReturnTo(path)).toBe(path);
  });
  it("rejects external URLs and unrelated routes", () => {
    for (const path of ["https://evil.example", "//evil.example", "/login", `/resumes/${id}/../../login`, `/resumes/${id}\\evil`, undefined]) {
      expect(resumeReturnTo(path)).toBeUndefined();
    }
  });
});
