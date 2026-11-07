import { expect, type Page } from "@playwright/test";

const api = process.env.PLAYWRIGHT_API ?? "http://127.0.0.1:4181";
const app = process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:4173";

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
  return {
    id: input.id,
    title: input.filename,
    created_at: "2026-11-07T18:00:00Z",
    versions: [
      {
        id: `${input.id}-v1`,
        number: 1,
        filename: input.filename,
        media_type: input.filename.endsWith(".docx")
          ? "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
          : "application/pdf",
        status: input.status,
        created_at: "2026-11-07T18:00:00Z",
        parse: input.parse,
        runs: input.runs,
      },
    ],
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

export type ApiOptions = {
  signedIn?: boolean;
  resumes?: Array<typeof designed>;
  magicLinkStatus?: number;
  verifyStatus?: number;
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
    update: async (data: { resumes?: Array<typeof designed>; onConnectResumes?: Array<typeof designed>; event?: string; disconnect?: boolean }) => {
      const result = await page.request.post(`${api}/__seed/${id}`, { data });
      if (!result.ok()) throw new Error(`update failed: ${result.status()}`);
    },
    stats: async () => {
      const result = await page.request.get(`${api}/__seed/${id}`);
      return await result.json() as { connections: number; streams: number; detailReads: number };
    },
  };
}
