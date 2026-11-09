import type { ParseInput, SectionKind } from "~/server/resumes";

/// Heading words each canonical section can appear under. A provider's extract
/// and its kept list both use free text, so compare by phrase, not equality.
const ALIASES: Record<SectionKind, string[]> = {
  summary: [
    "summary",
    "professional summary",
    "profile",
    "about",
    "about me",
    "objective",
  ],
  experience: [
    "experience",
    "work experience",
    "professional experience",
    "employment",
    "employment history",
    "work history",
    "career",
  ],
  education: ["education", "academic background", "education and training"],
  skills: [
    "skills",
    "technical skills",
    "core skills",
    "skill set",
    "technologies",
    "expertise",
    "competencies",
  ],
  projects: ["projects", "personal projects", "selected projects", "side projects"],
  certifications: [
    "certifications",
    "certificates",
    "certification",
    "licenses",
    "licenses and certifications",
  ],
  awards: ["awards", "honors", "honours", "awards and honors", "achievements"],
  languages: ["languages", "language"],
  publications: ["publications", "papers", "research"],
  volunteer: [
    "volunteer",
    "volunteering",
    "volunteer experience",
    "community",
    "community service",
  ],
  interests: ["interests", "hobbies", "hobbies and interests"],
  contact: [
    "contact",
    "contact information",
    "contact details",
    "email",
    "phone",
  ],
  other: [],
};

const LABELS: Record<SectionKind, string> = {
  summary: "Summary",
  experience: "Experience",
  education: "Education",
  skills: "Skills",
  projects: "Projects",
  certifications: "Certifications",
  awards: "Awards",
  languages: "Languages",
  publications: "Publications",
  volunteer: "Volunteer",
  interests: "Interests",
  contact: "Contact",
  other: "Other",
};

export function sectionKindLabel(kind: SectionKind): string {
  return LABELS[kind];
}

export function normalizeHeading(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function phraseIn(text: string, phrase: string): boolean {
  return ` ${text} `.includes(` ${phrase} `);
}

/// True when a free-text heading (or a verdict line) names this section.
export function headingMatches(kind: SectionKind, value: string): boolean {
  if (kind === "other") return false;
  const text = normalizeHeading(value);
  if (!text) return false;
  return ALIASES[kind].some((alias) => phraseIn(text, alias));
}

/// Whether one provider's own extraction found this section, looking at both
/// its section headings and the lines it reports as kept.
export function providerFound(
  kind: SectionKind,
  input: { parse?: ParseInput | null; kept: string[] },
): boolean {
  if (kind === "other") return false;
  const headings = input.parse?.profile.sections.map((section) => section.heading) ?? [];
  return [...headings, ...input.kept].some((value) => headingMatches(kind, value));
}
