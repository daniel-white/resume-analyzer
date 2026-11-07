import { apiFetch } from "./session";
import { medianScore } from "../lib/grades";

export type ResumeStatus = "uploaded" | "parsed" | "running" | "complete";
export type RunStatus = "requested" | "complete";
export type DateStyle = "mm-yyyy" | "other" | "absent";
export type LayoutConfidence = "structural" | "inferred";

export type Section = {
  heading: string;
  body: string;
  in_table: boolean;
  in_text_box: boolean;
  in_header: boolean;
};

export type Profile = {
  name: string | null;
  email: string | null;
  phone: string | null;
  sections: Section[];
};

export type Layout = {
  columns: number;
  contact_in_header: boolean;
  has_tables: boolean;
  has_text_boxes: boolean;
  has_images: boolean;
  bullet_count: number;
  dates: DateStyle;
};

export type ParseInput = {
  profile: Profile;
  layout: Layout;
  layout_confidence: LayoutConfidence;
};

export type Factor = {
  label: string;
  delta: number;
  advice: string;
};

export type Verdict = {
  kept: string[];
  dropped: string[];
  score: number;
  summary: string;
  factors: Factor[];
};

export type VersionSummary = {
  id: string;
  number: number;
  filename: string;
  status: ResumeStatus;
  created_at: string;
};

export type ComparisonVersion = VersionSummary & {
  scores: { id: string; name: string; score: number | null }[];
};

export type ResumeSummary = {
  id: string;
  title: string;
  created_at: string;
  latest: VersionSummary;
};

export type RunView = {
  id: string;
  plugin_id: string;
  status: RunStatus;
  parse: ParseInput | null;
  verdict: Verdict | null;
};

export type VersionDetail = {
  id: string;
  number: number;
  filename: string;
  media_type: string;
  status: ResumeStatus;
  created_at: string;
  parse: ParseInput | null;
  runs: RunView[];
};

export type ResumeDetail = {
  id: string;
  title: string;
  created_at: string;
  versions: VersionDetail[];
};

export type Plugin = {
  id: string;
  name: string;
  subtitle: string;
};

export type AcceptedUpload = {
  id: string;
  version_id: string;
  status: string;
};

export type ScoreBand = "low" | "mid" | "high";

export type LibraryRow = { overallScore: number | null } & (
  | {
      id: string;
      filename: string;
      status: "waiting";
      label: "Stored. Waiting for the first read.";
    }
  | {
      id: string;
      filename: string;
      status: "reading";
      label: "Reading";
    }
  | {
      id: string;
      filename: string;
      status: "scored";
      reads: number;
      readsLabel: string;
      low: number;
      high: number;
      spread: string;
      band: ScoreBand;
    });

export type DropCopy = {
  title: string;
  hint: string | null;
  tone: "empty" | "waiting" | "list";
};

export type LibraryView = {
  rows: LibraryRow[];
  drop: DropCopy;
};

export type ExtractField = {
  label: string;
  value: string;
};

export type FactorLine = {
  label: string;
  delta: string;
  advice?: string;
};

export type VerdictCard =
  | {
      id: string;
      name: string;
      subtitle: string;
      pending: true;
    }
  | {
      id: string;
      name: string;
      subtitle: string;
      pending: false;
      score: number;
      band: ScoreBand;
      kept: string[];
      missed: string[];
      factors: FactorLine[];
      summary?: string;
      parse?: ParseInput | null;
    };

export type Teardown =
  | { kind: "missing" }
  | {
      kind: "reading" | "complete";
      resumeId: string;
      versionId: string;
      versions: VersionSummary[];
      comparisonVersions: ComparisonVersion[];
      filename: string;
      lede: string;
      file: { url: string; mediaType: string };
      extract: ExtractField[] | null;
      parse?: ParseInput | null;
      cards: VerdictCard[];
    };

export type UploadResult =
  | { ok: true; id: string; version_id: string }
  | { ok: false; message: string };

const READING_COUNTS = [
  "No",
  "One",
  "Two",
  "Three",
  "Four",
  "Five",
  "Six",
  "Seven",
  "Eight",
];

export function readingLede(count: number): string {
  const word = READING_COUNTS[count] ?? String(count);
  if (count === 1) {
    return "One system is reading this file. It does not share a parse.";
  }
  return `${word} systems are reading this file. They do not share a parse.`;
}

const EMPTY = "\u2014";

export function scoreBand(score: number): ScoreBand {
  if (score >= 75) return "high";
  if (score >= 50) return "mid";
  return "low";
}

export function formatDelta(delta: number): string {
  if (delta > 0) return `+${delta}`;
  return String(delta);
}

export function libraryRow(summary: ResumeSummary, runs: RunView[]): LibraryRow {
  const id = summary.id;
  const filename = summary.latest.filename;
  const scores = runs.flatMap((run) => run.verdict ? [run.verdict.score] : []);
  const overallScore = medianScore(scores);
  if (summary.latest.status === "uploaded") {
    return { id, filename, overallScore, status: "waiting", label: "Stored. Waiting for the first read." };
  }
  if (summary.latest.status !== "complete") {
    return { id, filename, overallScore, status: "reading", label: "Reading" };
  }
  if (scores.length === 0) {
    return { id, filename, overallScore, status: "reading", label: "Reading" };
  }
  const low = Math.min(...scores);
  const high = Math.max(...scores);
  const reads = scores.length;
  return {
    id,
    filename,
    overallScore,
    status: "scored",
    reads,
    readsLabel: reads === 1 ? "1 read" : `${reads} reads`,
    low,
    high,
    spread: low === high ? String(low) : `${low}\u2013${high}`,
    band: scoreBand(low),
  };
}

export function dropCopy(rows: LibraryRow[]): DropCopy {
  if (rows.length === 0) {
    return {
      title: "Drop a PDF or DOCX",
      hint: "Each system reads the file itself. Five megabytes.",
      tone: "empty",
    };
  }
  if (rows.every((row) => row.status === "waiting")) {
    return { title: "Drop another file", hint: null, tone: "waiting" };
  }
  return {
    title: "Drop a PDF or DOCX",
    hint: "Each system reads the file itself. Five megabytes.",
    tone: "list",
  };
}

export function libraryView(rows: LibraryRow[]): LibraryView {
  return { rows, drop: dropCopy(rows) };
}

export function extractFields(parse: ParseInput): ExtractField[] {
  return [
    { label: "Name", value: parse.profile.name?.trim() || EMPTY },
    { label: "Email", value: parse.profile.email?.trim() || EMPTY },
    { label: "Layout", value: layoutLine(parse.layout) },
    { label: "Sections", value: sectionLine(parse.profile.sections) },
  ];
}

export function completeLede(parse: ParseInput | null): string {
  if (!parse) return "Each system opened the file itself.";
  const { columns, contact_in_header, has_tables } = parse.layout;
  if (columns === 1 && !contact_in_header && !has_tables) {
    return "One column. Contact in the body. Each system opened the file itself.";
  }
  const parts = [
    contact_in_header ? "Contact in the header" : "Contact in the body",
  ];
  if (has_tables) parts.push("skills in a table");
  parts.push(columnPhrase(columns));
  return `${parts.join(", ")}. Each system opened the file itself.`;
}

export function pluginCards(plugins: Plugin[], runs: RunView[]): VerdictCard[] {
  return plugins.map((plugin) => {
    const verdict = runs.find((run) => run.plugin_id === plugin.id)?.verdict;
    if (!verdict) {
      return {
        id: plugin.id,
        name: plugin.name,
        subtitle: plugin.subtitle,
        pending: true,
      };
    }
    return {
      id: plugin.id,
      name: plugin.name,
      subtitle: plugin.subtitle,
      pending: false,
      score: verdict.score,
      band: scoreBand(verdict.score),
      kept: verdict.kept,
      missed: verdict.dropped,
      factors: verdict.factors.map((factor) => ({
        label: factor.label,
        delta: formatDelta(factor.delta),
        advice: factor.advice,
      })),
      summary: verdict.summary,
      parse: runs.find((run) => run.plugin_id === plugin.id)?.parse ?? null,
    };
  });
}

export function teardown(detail: ResumeDetail, plugins: Plugin[], versionId?: string): Teardown {
  const version = versionId
    ? detail.versions.find((item) => item.id === versionId)
    : detail.versions[0];
  if (!version) return { kind: "missing" };
  const reading = version.status !== "complete";
  return {
    kind: reading ? "reading" : "complete",
    resumeId: detail.id,
    versionId: version.id,
    versions: detail.versions.map(({ id, number, filename, status, created_at }) => ({
      id, number, filename, status, created_at,
    })),
    comparisonVersions: detail.versions.map((item) => ({
      id: item.id, number: item.number, filename: item.filename,
      status: item.status, created_at: item.created_at,
      scores: pluginCards(plugins, item.runs).map((card) => ({
        id: card.id, name: card.name, score: card.pending ? null : card.score,
      })),
    })),
    filename: version.filename,
    file: {
      url: `/api/resumes/${encodeURIComponent(detail.id)}/versions/${encodeURIComponent(version.id)}/file`,
      mediaType: version.media_type,
    },
    lede: reading ? readingLede(plugins.length) : completeLede(version.parse),
    extract: version.parse ? extractFields(version.parse) : null,
    parse: version.parse,
    cards: pluginCards(plugins, version.runs),
  };
}

export function uploadFailure(status: number, body: string): string {
  const text = body.trim();
  if (text && text.length <= 200 && !text.startsWith("{")) return text;
  if (status === 415) return "That file is neither a PDF nor a DOCX.";
  if (status === 413) return "That file is over five megabytes.";
  return "That file did not store.";
}

export async function fetchLibrary(
  baseUrl: string,
  cookie: string | undefined,
): Promise<LibraryView> {
  const list = await fetchResumeList(baseUrl, cookie);
  const rows = await Promise.all(
    list.map(async (summary) => {
      const detail = await fetchResume(baseUrl, summary.id, cookie);
      const version = detail?.versions.find((item) => item.id === summary.latest.id);
      return libraryRow(summary, version?.runs ?? []);
    }),
  );
  return libraryView(rows);
}

export async function fetchTeardown(
  baseUrl: string,
  id: string,
  cookie: string | undefined,
  versionId?: string,
): Promise<Teardown> {
  const [detail, plugins] = await Promise.all([
    fetchResume(baseUrl, id, cookie),
    fetchPlugins(baseUrl),
  ]);
  if (!detail) return { kind: "missing" };
  return teardown(detail, plugins, versionId);
}

export async function postResume(
  baseUrl: string,
  file: File,
  cookie: string | undefined,
  resumeId?: string,
): Promise<UploadResult> {
  if (file.size === 0) {
    return { ok: false, message: "Choose a PDF or DOCX." };
  }
  const body = new FormData();
  body.set("file", file, file.name);
  const path = resumeId ? `/api/resumes/${encodeURIComponent(resumeId)}/versions` : "/api/resumes";
  const result = await apiFetch(baseUrl, path, cookie, { method: "POST", body });
  if (!result.ok) {
    return { ok: false, message: uploadFailure(result.status, result.body) };
  }
  const accepted = (await result.value.json()) as AcceptedUpload;
  return { ok: true, id: accepted.id, version_id: accepted.version_id };
}

export type DeleteResult = { ok: true } | { ok: false; message: string };

export async function deleteResume(
  baseUrl: string,
  id: string,
  cookie: string | undefined,
  versionId?: string,
): Promise<DeleteResult> {
  const path = `/api/resumes/${encodeURIComponent(id)}`
    + (versionId ? `/versions/${encodeURIComponent(versionId)}` : "");
  const result = await apiFetch(baseUrl, path, cookie, { method: "DELETE" });
  if (!result.ok) {
    return { ok: false, message: result.status === 404
      ? "That resume version is no longer here. Refresh and try again."
      : "Could not delete the resume. Please try again." };
  }
  return { ok: true };
}

async function fetchResumeList(
  baseUrl: string,
  cookie: string | undefined,
): Promise<ResumeSummary[]> {
  const result = await apiFetch(baseUrl, "/api/resumes", cookie);
  if (!result.ok) throw new Error(`resumes returned ${result.status}`);
  const body = (await result.value.json()) as { resumes: ResumeSummary[] };
  return body.resumes;
}

async function fetchResume(
  baseUrl: string,
  id: string,
  cookie: string | undefined,
): Promise<ResumeDetail | null> {
  const result = await apiFetch(baseUrl, `/api/resumes/${id}`, cookie);
  if (!result.ok) {
    if (result.status === 404) return null;
    throw new Error(`resume ${id} returned ${result.status}`);
  }
  return (await result.value.json()) as ResumeDetail;
}

async function fetchPlugins(baseUrl: string): Promise<Plugin[]> {
  const result = await apiFetch(baseUrl, "/api/plugins", undefined);
  if (!result.ok) throw new Error(`plugins returned ${result.status}`);
  const body = (await result.value.json()) as { plugins: Plugin[] };
  return body.plugins;
}

function layoutLine(layout: Layout): string {
  const columns = layout.columns === 1 ? "1 column" : `${layout.columns} columns`;
  const contact = layout.contact_in_header ? "contact in the header" : "contact in the body";
  const parts = [columns, contact];
  if (layout.has_tables) parts.push("skills in a table");
  return parts.join(", ");
}

function sectionLine(sections: Section[]): string {
  const headings = sections.map((section) => section.heading.trim()).filter(Boolean);
  if (headings.length === 0) return EMPTY;
  return headings.join(", ");
}

function columnPhrase(columns: number): string {
  if (columns === 1) return "one column";
  if (columns === 2) return "two columns";
  return `${columns} columns`;
}
