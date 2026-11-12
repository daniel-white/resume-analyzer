import { describe, expect, it } from "vitest";

import type { AdminFeedback } from "~/server/admin";
import { adminStats, feedbackHandled, filterFeedback } from "./admin";

function item(overrides: Partial<AdminFeedback> = {}): AdminFeedback {
  return {
    version_id: "version-1",
    user_id: "user-1",
    user_email: "ada@example.com",
    resume_id: "resume-1",
    resume_title: "ada.pdf",
    version_number: 1,
    filename: "ada.pdf",
    section_id: "experience",
    section_heading: "Experience",
    section_kind: "experience",
    section_body: "Built small systems.",
    voted_at: "2026-01-01T00:00:00Z",
    analyses: [],
    ...overrides,
  };
}

function diagnosis(id: string, handled = false) {
  return {
    id,
    model: "llama-3.2",
    engine: "llama.cpp",
    diagnosis: "The body reads as a fragment.",
    created_at: "2026-01-01T00:00:00Z",
    ...(handled ? { handled_at: "2026-01-02T00:00:00Z", handled_by: "ada@admin.example.com" } : {}),
  };
}

describe("admin feedback", () => {
  it("counts people, readings, and open items", () => {
    const feedback = [
      item({ analyses: [diagnosis("a")] }),
      item({ section_id: "summary", analyses: [diagnosis("b", true)] }),
      item({ user_id: "user-2", analyses: [] }),
    ];
    expect(adminStats(feedback)).toEqual({
      items: 3,
      open: 2,
      handled: 1,
      analyses: 2,
      users: 2,
      sections: 2,
    });
  });

  it("treats a downvote without a diagnosis as open", () => {
    expect(feedbackHandled(item())).toBe(false);
    expect(feedbackHandled(item({ analyses: [diagnosis("a", true)] }))).toBe(true);
    expect(feedbackHandled(item({ analyses: [diagnosis("a", true), diagnosis("b")] }))).toBe(false);
  });

  it("filters by handling state and free text", () => {
    const feedback = [
      item({ analyses: [diagnosis("a")] }),
      item({ section_id: "summary", section_heading: "Profile impression", analyses: [diagnosis("b", true)] }),
    ];
    expect(filterFeedback(feedback, "open").map((entry) => entry.section_id)).toEqual(["experience"]);
    expect(filterFeedback(feedback, "handled").map((entry) => entry.section_id)).toEqual(["summary"]);
    expect(filterFeedback(feedback, "all", "PROFILE").map((entry) => entry.section_id)).toEqual(["summary"]);
    expect(filterFeedback(feedback, "all", "fragment")).toHaveLength(2);
    expect(filterFeedback(feedback, "all", "nobody")).toHaveLength(0);
  });
});
