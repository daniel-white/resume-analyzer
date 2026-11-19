import { expect, type Page } from "@playwright/test";

const api = process.env["PLAYWRIGHT_API"] ?? "http://127.0.0.1:4181";
const app = process.env["PLAYWRIGHT_BASE_URL"] ?? "http://127.0.0.1:4173";

type Verdict = {
  kept: string[];
  dropped: string[];
  score: number;
  summary: string;
  factors: { label: string; delta: number; advice: string }[];
};

function verdict(score: number, kept: string[], dropped: string[]): Verdict {
  return {
    kept,
    dropped,
    score,
    summary: `Score ${score} of 100.`,
    factors: [
      { label: "contact in header", delta: -25, advice: "Move the email into the body." },
      { label: "standard headings", delta: 10, advice: "Keep the standard headings." },
    ],
  };
}

const designedParse = {
  profile: {
    name: "Ada Lovelace",
    email: "ada@example.com",
    phone: null,
    sections: [
      {
        heading: "Experience",
        body: "Analyst",
        in_table: false,
        in_text_box: false,
        in_header: false,
      },
      {
        heading: "Skills",
        body: "Analytical Engines",
        in_table: true,
        in_text_box: false,
        in_header: false,
      },
    ],
  },
  layout: {
    columns: 2,
    contact_in_header: true,
    has_tables: true,
    has_text_boxes: false,
    has_images: true,
    bullet_count: 2,
    dates: "other" as const,
  },
  layout_confidence: "structural" as const,
};

const designedSections = {
  engine: "sections-v1",
  model: "qwen2.5:0.5b",
  sections: [
    {
      id: "experience-1",
      kind: "experience" as const,
      heading: "Experience",
      body: "Analyst",
      confidence: 0.9,
      feedback: null,
    },
    {
      id: "skills-1",
      kind: "skills" as const,
      heading: "Skills",
      body: "Analytical Engines, Data visualization",
      confidence: 0.9,
      feedback: null,
    },
  ],
};

/** Deliberately spans every job-agnostic category and contextual nature. */
export const designedSkills = {
  engine: "skills-v1",
  model: "qwen2.5:0.5b",
  skills: [
    {
      name: "Analytical engines",
      canonical: "analytical-engines",
      category: "tool" as const,
      nature: "technical" as const,
      importance: 88,
      confidence: 92,
      evidence: [{ section_id: "skills-1", quote: "Analytical Engines" }],
    },
    {
      name: "Stakeholder education",
      canonical: "stakeholder-education",
      category: "practice" as const,
      nature: "people" as const,
      importance: 76,
      confidence: 84,
      evidence: [{ section_id: "experience-1", quote: "Analyst" }],
    },
    {
      name: "Clear technical writing",
      canonical: "clear-technical-writing",
      category: "craft" as const,
      nature: "soft" as const,
      importance: 64,
      confidence: 78,
      evidence: [{ section_id: "experience-1", quote: "Analyst" }],
    },
    {
      name: "Risk analysis",
      canonical: "risk-analysis",
      category: "method" as const,
      nature: "hard" as const,
      importance: 80,
      confidence: 88,
      evidence: [{ section_id: "experience-1", quote: "Analyst" }],
    },
    {
      name: "Computing history",
      canonical: "computing-history",
      category: "domain" as const,
      nature: "domain" as const,
      importance: 52,
      confidence: 70,
      evidence: [{ section_id: "experience-1", quote: "Analyst" }],
    },
    {
      name: "Data visualization",
      canonical: "data-visualization",
      category: "creative" as const,
      nature: "hard" as const,
      importance: 61,
      confidence: 65,
      evidence: [{ section_id: "skills-1", quote: "Data visualization" }],
    },
  ],
};

function run(pluginId: string, scored: Verdict | null, parse: typeof designedParse | null = null) {
  return {
    id: `run-${pluginId}`,
    plugin_id: pluginId,
    status: scored ? ("complete" as const) : ("requested" as const),
    parse,
    verdict: scored,
  };
}

function resume(input: {
  id: string;
  filename: string;
  status: "uploaded" | "parsed" | "running" | "complete";
  parse: typeof designedParse | null;
  runs: ReturnType<typeof run>[];
}) {
  const version = {
    id: `${input.id}-v1`,
    number: 1,
    filename: input.filename,
    media_type: input.filename.endsWith(".docx")
      ? "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
      : "application/pdf",
    status: input.status,
    created_at: "2026-11-07T18:00:00Z",
    parse: input.parse,
    profile_summary: input.parse ? {
      text: "Ada Lovelace is a computing pioneer with skills in analytical engines and mathematics.",
      engine: "ollama",
      model: "qwen2.5:0.5b",
      feedback: null,
    } : null,
    runs: input.runs,
    sections: input.parse ? designedSections : null,
    skills: input.parse ? designedSkills : null,
  };
  return {
    id: input.id,
    title: input.filename,
    created_at: "2026-11-07T18:00:00Z",
    versions: [version] as [typeof version],
  };
}

export const designed = resume({
  id: "resume-designed",
  filename: "ada-designed.pdf",
  status: "complete",
  parse: designedParse,
  runs: [
    run("greenline", verdict(35, ["Experience"], ["header contact", "table"])),
    run("levr", verdict(70, ["experience bullets"], ["skills"])),
  ],
});

export const reading = resume({
  id: "resume-reading",
  filename: "workshop.pdf",
  status: "running",
  parse: null,
  runs: [],
});

export const stored = resume({
  id: "resume-stored",
  filename: "just-added.docx",
  status: "uploaded",
  parse: null,
  runs: [],
});

type SeedVersion = (typeof designed)["versions"][number];
export type SeedResume = Omit<typeof designed, "versions"> & { versions: SeedVersion[] };

/** A GenerationStream frame the stub can push to every open SSE connection. */
export type SeedFrame = {
  resume_id: string;
  version_id: string;
  flow: "summary" | "feedback" | "skills";
  section_id: string | null;
  seq: number;
  event: { type: string } & Record<string, unknown>;
};

export type ApiOptions = {
  signedIn?: boolean;
  resumes?: SeedResume[];
  magicLinkStatus?: number;
  verifyStatus?: number;
  admin?: boolean;
  adminFeedback?: unknown[];
};

export async function ready(page: Page) {
  await expect(page.locator("main")).toBeVisible();
  await page.waitForFunction(() =>
    Object.keys(document.body).some((key) => key.startsWith("__react")),
  );
}

export async function installApi(page: Page, options: ApiOptions = {}) {
  const response = await page.request.post(`${api}/__seed`, { data: options });
  if (!response.ok()) {
    throw new Error(`seed failed: ${response.status()} ${await response.text()}`);
  }
  const { id } = (await response.json()) as { id: string };
  await page.context().addCookies([
    {
      name: "pw",
      value: id,
      url: app,
    },
  ]);
  return {
    update: async (data: {
      resumes?: SeedResume[];
      onConnectResumes?: SeedResume[];
      event?: string;
      frames?: SeedFrame[];
      disconnect?: boolean;
    }) => {
      const result = await page.request.post(`${api}/__seed/${id}`, { data });
      if (!result.ok()) throw new Error(`update failed: ${result.status()}`);
    },
    stats: async () => {
      const result = await page.request.get(`${api}/__seed/${id}`);
      return await result.json() as { connections: number; streams: number; detailReads: number };
    },
  };
}
