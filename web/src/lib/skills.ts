import type { LiveSkill } from "./generation";
import type {
  SectionMap,
  Skill,
  SkillCategory,
  SkillNature,
  SkillSet,
  SkillSummary,
} from "~/server/resumes";

/** One tag as the cloud renders it. Live tags are provisional until stored. */
export type CloudSkill = {
  key: string;
  name: string;
  category: SkillCategory;
  nature: SkillNature;
  importance: number;
  confidence: number;
  live: boolean;
};

/** Job-agnostic: these buckets fit a nurse, a teacher, or a developer alike. */
export const CATEGORIES: SkillCategory[] = [
  "tool",
  "method",
  "craft",
  "practice",
  "domain",
  "creative",
  "other",
];

/** The contextual second axis, independent of occupation. */
export const NATURES: SkillNature[] = ["technical", "hard", "people", "soft", "domain"];

export const CATEGORY_LABELS: Record<SkillCategory, string> = {
  tool: "Tools",
  method: "Methods",
  craft: "Craft",
  practice: "Practice",
  domain: "Domain",
  creative: "Creative",
  other: "Other",
};

export const NATURE_LABELS: Record<SkillNature, string> = {
  technical: "Technical",
  hard: "Hard",
  people: "People",
  soft: "Soft",
  domain: "Domain",
};

const LIST_ONLY_KINDS = new Set(["skills", "interests", "languages"]);

export function category(value: string): SkillCategory {
  return CATEGORIES.includes(value as SkillCategory) ? (value as SkillCategory) : "tool";
}

/** Mirror the Rust parser and its category-based fallback for streamed tags. */
export function nature(value: string, fallback: SkillCategory): SkillNature {
  const parsed = value.trim().toLowerCase();
  if (parsed === "technical" || parsed === "tech") return "technical";
  if (parsed === "hard") return "hard";
  if (parsed === "people" || parsed === "leadership") return "people";
  if (parsed === "soft") return "soft";
  if (parsed === "domain") return "domain";
  if (fallback === "tool") return "technical";
  if (fallback === "domain") return "domain";
  if (fallback === "practice" || fallback === "other") return "soft";
  return "hard";
}

export function storedCloud(set: SkillSet | null): CloudSkill[] {
  return (set?.skills ?? []).map((skill) => ({
    key: skill.canonical || skill.name.toLowerCase(),
    name: skill.name,
    category: skill.category,
    nature: skill.nature,
    importance: skill.importance,
    confidence: skill.confidence,
    live: false,
  }));
}

/** Streamed tags size by the model's prominence; the stored map has the final score. */
export function liveCloud(skills: LiveSkill[]): CloudSkill[] {
  return skills.map((skill) => {
    const fallback = category(skill.category);
    return {
      key: skill.key,
      name: skill.name,
      category: fallback,
      nature: nature(skill.nature, fallback),
      importance: Math.min(100, skill.prominence * 20),
      confidence: skill.confidence,
      live: true,
    };
  });
}

export function filterCloud(
  tags: CloudSkill[],
  filter: SkillNature | "all",
): CloudSkill[] {
  return filter === "all" ? tags : tags.filter((tag) => tag.nature === filter);
}

export function tagStyle(skill: CloudSkill): { fontSize: string; opacity: number } {
  return {
    fontSize: `${(0.78 + (skill.importance / 100) * 0.62).toFixed(2)}rem`,
    opacity: 0.62 + (skill.confidence / 100) * 0.38,
  };
}

/** A skill whose only proof is a list entry still needs a real example. */
export function needsEvidence(skill: Skill, sections: SectionMap | null): boolean {
  if (skill.evidence.length === 0) return true;
  return skill.evidence.every((item) => {
    const section = sections?.sections.find((entry) => entry.id === item.section_id);
    return section ? LIST_ONLY_KINDS.has(section.kind) : true;
  });
}

export function skillDiff(from: SkillSummary[], to: SkillSummary[]) {
  const before = new Map(from.map((skill) => [skill.canonical, skill]));
  const after = new Map(to.map((skill) => [skill.canonical, skill]));
  const added = to.filter((skill) => !before.has(skill.canonical));
  const removed = from.filter((skill) => !after.has(skill.canonical));
  const moved = to.flatMap((skill) => {
    const previous = before.get(skill.canonical);
    const delta = previous ? skill.importance - previous.importance : 0;
    return previous && Math.abs(delta) >= 10 ? [{ ...skill, delta }] : [];
  });
  return { added, removed, moved };
}
