import { describe, expect, it } from "vitest";

import { headingMatches, normalizeHeading, providerFound, sectionKindLabel } from "./sections";

describe("section heading matching", () => {
  it("maps common heading variations onto canonical kinds", () => {
    expect(headingMatches("experience", "Work Experience")).toBe(true);
    expect(headingMatches("experience", "PROFESSIONAL EXPERIENCE")).toBe(true);
    expect(headingMatches("skills", "Technical Skills")).toBe(true);
    expect(headingMatches("education", "Education & Training")).toBe(true);
    expect(headingMatches("contact", "ada@example.com")).toBe(false);
    expect(headingMatches("contact", "email")).toBe(true);
    expect(headingMatches("languages", "Language")).toBe(true);
  });

  it("does not turn unrelated lines into a match", () => {
    expect(headingMatches("experience", "experience bullets")).toBe(true);
    expect(headingMatches("other", "Experience")).toBe(false);
    expect(headingMatches("skills", "table")).toBe(false);
    expect(headingMatches("summary", "headings")).toBe(false);
    expect(normalizeHeading("  Work—Experience  ")).toBe("work experience");
  });
});

describe("provider section detection", () => {
  it("uses the provider's own extract and kept lines", () => {
    expect(providerFound("experience", { kept: ["Experience"] })).toBe(true);
    expect(providerFound("experience", { kept: ["experience bullets"] })).toBe(true);
    expect(
      providerFound("education", {
        kept: [],
        parse: {
          profile: {
            name: null,
            email: null,
            phone: null,
            sections: [
              { heading: "Education", body: "", in_table: false, in_text_box: false, in_header: false },
            ],
          },
          layout: {
            columns: 1,
            contact_in_header: false,
            has_tables: false,
            has_text_boxes: false,
            has_images: false,
            bullet_count: 0,
            dates: "absent",
          },
          layout_confidence: "structural",
        },
      }),
    ).toBe(true);
    expect(providerFound("projects", { kept: ["email", "Experience"] })).toBe(false);
  });

  it("labels every kind for the comparison list", () => {
    expect(sectionKindLabel("certifications")).toBe("Certifications");
    expect(sectionKindLabel("volunteer")).toBe("Volunteer");
  });
});
