import { describe, expect, it } from "vitest";

import type { Skill, SkillSet } from "~/server/resumes";
import {
  CATEGORIES,
  NATURES,
  filterCloud,
  liveCloud,
  nature,
  needsEvidence,
  skillDiff,
  storedCloud,
  tagStyle,
} from "./skills";

const stored: SkillSet = {
  engine: "skills-v1",
  model: "test",
  skills: [
    {
      name: "Patient education",
      canonical: "patient-education",
      category: "practice",
      nature: "people",
      importance: 82,
      confidence: 90,
      evidence: [{ section_id: "experience-1", quote: "Taught patients." }],
    },
    {
      name: "Box office reconciliation",
      canonical: "box-office-reconciliation",
      category: "method",
      nature: "hard",
      importance: 44,
      confidence: 60,
      evidence: [{ section_id: "skills-1", quote: "Reconciliation" }],
    },
  ],
};

function fixtureSkill(index: number): Skill {
  const skill = stored.skills[index];
  if (!skill) throw new Error("missing fixture skill");
  return skill;
}

describe("skill cloud helpers", () => {
  it("covers every job-agnostic category and contextual nature", () => {
    expect(CATEGORIES).not.toContain("language");
    expect(CATEGORIES).not.toContain("framework");
    expect(CATEGORIES).toEqual([
      "tool", "method", "craft", "practice", "domain", "creative", "other",
    ]);
    expect(NATURES).toEqual(["technical", "hard", "people", "soft", "domain"]);
  });

  it("normalizes streamed skills and derives nature when the model omits it", () => {
    const tags = liveCloud([
      {
        key: "triage",
        name: "Triage",
        category: "method",
        nature: "hard",
        prominence: 5,
        confidence: 92,
      },
      {
        key: "excel",
        name: "Excel",
        category: "tool",
        nature: "",
        prominence: 2,
        confidence: 50,
      },
      {
        key: "hospitality",
        name: "Hospitality",
        category: "unknown",
        nature: "leadership",
        prominence: 3,
        confidence: 70,
      },
    ]);
    expect(tags.map((tag) => [tag.nature, tag.category, tag.importance])).toEqual([
      ["hard", "method", 100],
      ["technical", "tool", 40],
      ["people", "tool", 60],
    ]);
    expect(tags.every((tag) => tag.live)).toBe(true);
    const first = tags[0];
    expect(first).toBeDefined();
    if (first) expect(tagStyle(first).fontSize).toBe("1.40rem");
  });

  it("normalizes nature aliases and falls back from non-software categories", () => {
    expect(nature("tech", "craft")).toBe("technical");
    expect(nature("Leadership", "other")).toBe("people");
    expect(nature("nonsense", "domain")).toBe("domain");
    expect(nature("", "practice")).toBe("soft");
    expect(nature("", "creative")).toBe("hard");
  });

  it("carries stored nature into the cloud and filters by it", () => {
    const tags = storedCloud(stored);
    expect(tags.map((tag) => tag.nature)).toEqual(["people", "hard"]);
    expect(tags.every((tag) => !tag.live)).toBe(true);
    expect(filterCloud(tags, "people").map((tag) => tag.name)).toEqual(["Patient education"]);
    expect(filterCloud(tags, "all")).toHaveLength(2);
  });

  it("flags a skill whose only proof is a bare list entry", () => {
    const sections = {
      engine: "sections-v1",
      model: "test",
      sections: [
        {
          id: "experience-1",
          kind: "experience" as const,
          heading: "Experience",
          body: "Taught patients.",
          confidence: 0.9,
          feedback: null,
        },
        {
          id: "skills-1",
          kind: "skills" as const,
          heading: "Skills",
          body: "Reconciliation",
          confidence: 0.9,
          feedback: null,
        },
      ],
    };
    const proven = fixtureSkill(0);
    const listed = fixtureSkill(1);
    expect(needsEvidence(proven, sections)).toBe(false);
    expect(needsEvidence(listed, sections)).toBe(true);
    expect(needsEvidence({ ...proven, evidence: [] }, sections)).toBe(true);
  });

  it("diffs skills by canonical name, including importance shifts", () => {
    const before = [
      { name: "Triage", canonical: "triage", nature: "hard" as const, importance: 50 },
      { name: "Charting", canonical: "charting", nature: "hard" as const, importance: 70 },
    ];
    const after = [
      { name: "Triage", canonical: "triage", nature: "hard" as const, importance: 82 },
      { name: "Mentoring", canonical: "mentoring", nature: "people" as const, importance: 60 },
    ];
    const diff = skillDiff(before, after);
    expect(diff.added.map((skill) => skill.name)).toEqual(["Mentoring"]);
    expect(diff.removed.map((skill) => skill.name)).toEqual(["Charting"]);
    expect(diff.moved.map((skill) => skill.delta)).toEqual([32]);
  });
});
