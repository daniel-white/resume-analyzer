import { afterEach, describe, expect, it, vi } from "vitest";

import {
  completeLede,
  deleteResume,
  renameResume,
  fetchTeardown,
  postResume,
  stageResume,
  dropCopy,
  extractFields,
  libraryRow,
  pluginCards,
  readingLede,
  scoreBand,
  teardown,
  uploadFailure,
  voteSection,
  type ParseInput,
  type Plugin,
  type ResumeDetail,
  type ResumeStatus,
  type ResumeSummary,
  type RunView,
  type Verdict,
} from "./resumes";

describe("private anonymous staging", () => {
  afterEach(() => vi.restoreAllMocks());

  it("accepts a private upload without requiring a session", async () => {
    const mock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({ id: "pending-id" }), { status: 202 }));
    expect(await stageResume("http://localhost:8080", new File(["%PDF"], "resume.pdf"))).toEqual({ ok: true, id: "pending-id" });
    expect(String(mock.mock.calls[0][0])).toBe("http://localhost:8080/api/uploads");
    expect(mock.mock.calls[0][1]!.method).toBe("POST");
  });

  it("rejects empty and oversized uploads before contacting storage", async () => {
    const mock = vi.spyOn(globalThis, "fetch");
    expect(await stageResume("http://localhost:8080", new File([], "empty.pdf"))).toEqual({ ok: false, message: "Choose a PDF or DOCX." });
    expect(await stageResume("http://localhost:8080", new File([new Uint8Array(5 * 1024 * 1024 + 1)], "large.pdf"))).toEqual({ ok: false, message: "That file is over five megabytes." });
    expect(mock).not.toHaveBeenCalled();
  });

  it("surfaces server-side content validation failures", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("", { status: 415 }));
    expect(await stageResume("http://localhost:8080", new File(["fake"], "resume.pdf"))).toEqual({ ok: false, message: "That file is neither a PDF nor a DOCX." });
  });
});

describe("resume set names", () => {
  afterEach(() => vi.restoreAllMocks());

  it("patches a trimmed name with the owner's session", async () => {
    const mock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));
    expect(await renameResume("http://api.test", "resume-1", "  Engineering roles  ", "session=test"))
      .toEqual({ ok: true });
    const [url, options] = mock.mock.calls[0];
    expect(String(url)).toBe("http://api.test/api/resumes/resume-1");
    expect(options).toMatchObject({ method: "PATCH", body: JSON.stringify({ title: "Engineering roles" }) });
    expect(new Headers(options?.headers).get("cookie")).toBe("session=test");
  });

  it("rejects invalid names without contacting the API", async () => {
    const mock = vi.spyOn(globalThis, "fetch");
    for (const title of ["", "   ", "x".repeat(201)]) {
      expect(await renameResume("http://api.test", "resume-1", title, undefined))
        .toEqual({ ok: false, message: "Use a name between 1 and 200 characters." });
    }
    expect(mock).not.toHaveBeenCalled();
  });

  it("reports a deleted or unowned set without pretending it saved", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 404 }));
    expect(await renameResume("http://api.test", "resume-1", "Engineering", "session=test"))
      .toEqual({ ok: false, message: "That resume is no longer here. Refresh and try again." });
  });

  it("reports a failed save and preserves session expiry handling", async () => {
    const mock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(new Response(null, { status: 500 }))
      .mockResolvedValueOnce(new Response(null, { status: 401 }));
    expect(await renameResume("http://api.test", "resume-1", "Engineering", "session=test"))
      .toEqual({ ok: false, message: "Could not rename your resume. Please try again." });
    await expect(renameResume("http://api.test", "resume-1", "Engineering", "session=test"))
      .rejects.toThrow("signed out");
    expect(mock).toHaveBeenCalledTimes(2);
  });
});

const plugins: Plugin[] = [
  {
    id: "greenline",
    name: "Greenline",
    subtitle: "Standard headings and plain text. Ignores the header.",
  },
  {
    id: "levr",
    name: "Levr",
    subtitle: "Skills match only as exact tokens.",
  },
  {
    id: "workdaye",
    name: "Workdaye",
    subtitle: "Contact must be in the body. Dates must be MM/YYYY.",
  },
  {
    id: "ash",
    name: "Ash",
    subtitle: "Keeps more structure. Still drops images.",
  },
];

function summary(status: ResumeStatus, filename = "ada-designed.pdf"): ResumeSummary {
  return {
    id: "resume-1",
    title: filename,
    created_at: "2026-11-07T18:00:00Z",
    latest: {
      id: "version-2",
      number: 2,
      filename,
      status,
      created_at: "2026-11-07T18:00:00Z",
    },
  };
}

function verdict(score: number, kept: string[] = ["Experience"], dropped: string[] = ["table"]): Verdict {
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

function run(pluginId: string, score: number | null, id = `run-${pluginId}`): RunView {
  return {
    id,
    plugin_id: pluginId,
    status: score === null ? "requested" : "complete",
    parse: null,
    verdict: score === null ? null : verdict(score),
  };
}

function parse(options: {
  name?: string | null;
  email?: string | null;
  sections?: string[];
  columns?: number;
  contact_in_header?: boolean;
  has_tables?: boolean;
}): ParseInput {
  return {
    profile: {
      name: options.name === undefined ? "Ada Lovelace" : options.name,
      email: options.email === undefined ? "ada@example.com" : options.email,
      phone: null,
      sections: (options.sections ?? ["Experience", "Skills"]).map((heading) => ({
        heading,
        body: "",
        in_table: false,
        in_text_box: false,
        in_header: false,
      })),
    },
    layout: {
      columns: options.columns ?? 1,
      contact_in_header: options.contact_in_header ?? false,
      has_tables: options.has_tables ?? false,
      has_text_boxes: false,
      has_images: false,
      bullet_count: 0,
      dates: "absent",
    },
    layout_confidence: "inferred",
  };
}

function version(
  status: ResumeStatus,
  body: ParseInput | null,
  runs: RunView[],
  filename: string,
  number: number,
): ResumeDetail["versions"][number] {
  return {
    id: `version-${number}`,
    number,
    filename,
    media_type: "application/pdf",
    status,
    created_at: "2026-11-07T18:00:00Z",
    parse: body,
    runs,
  };
}

describe("score bands", () => {
  it("puts the cut between the mid 70 and the high 78", () => {
    expect(scoreBand(49)).toBe("low");
    expect(scoreBand(50)).toBe("mid");
    expect(scoreBand(74)).toBe("mid");
    expect(scoreBand(75)).toBe("high");
    expect(scoreBand(5)).toBe("low");
    expect(scoreBand(35)).toBe("low");
    expect(scoreBand(70)).toBe("mid");
    expect(scoreBand(78)).toBe("high");
    expect(scoreBand(95)).toBe("high");
  });
});

describe("library rows", () => {
  it("keeps the set name distinct from the latest file", () => {
    expect(libraryRow({ ...summary("uploaded", "revised.pdf"), title: "Engineering roles" }, []))
      .toMatchObject({ title: "Engineering roles", filename: "revised.pdf" });
  });

  it("says a stored file is waiting on the first read", () => {
    expect(libraryRow(summary("uploaded"), [])).toEqual({
      id: "resume-1",
      title: "ada-designed.pdf",
      filename: "ada-designed.pdf",
      status: "waiting",
      overallScore: null,
      label: "Stored. Waiting for the first read.",
    });
  });

  it("says parsed and running files are still reading", () => {
    expect(libraryRow(summary("parsed"), [run("greenline", null)]).status).toBe("reading");
    const running = libraryRow(summary("running"), [run("greenline", 35)]);
    expect(running.status === "reading" && running.label).toBe("Reading");
    expect(running.overallScore).toBe(35);
  });

  it("spreads the verdict scores and colors by the low one", () => {
    expect(
      libraryRow(summary("complete", "workshop.pdf"), [
        run("greenline", 35),
        run("levr", 70),
        run("workdaye", 5),
        run("ash", 78),
      ]),
    ).toMatchObject({
      filename: "workshop.pdf",
      status: "scored",
      reads: 4,
      readsLabel: "4 reads",
      low: 5,
      high: 78,
      spread: "5\u201378",
      band: "low",
      overallScore: 52.5,
    });
  });

  it("colors a high floor as high, including a single score", () => {
    const high = libraryRow(summary("complete", "ada-plain.docx"), [
      run("greenline", 95),
      run("levr", 100),
      run("workdaye", 100),
      run("ash", 95),
    ]);
    expect(high).toMatchObject({
      readsLabel: "4 reads",
      spread: "95\u2013100",
      band: "high",
    });

    expect(libraryRow(summary("complete"), [run("levr", 70)])).toMatchObject({
      reads: 1,
      readsLabel: "1 read",
      low: 70,
      high: 70,
      spread: "70",
      band: "mid",
    });
  });

  it("ignores a run that has no verdict yet", () => {
    expect(
      libraryRow(summary("complete"), [run("greenline", null), run("ash", 78)]).status,
    ).toBe("scored");
    expect(
      libraryRow(summary("complete"), [run("greenline", null), run("ash", 78)]),
    ).toMatchObject({ reads: 1, low: 78, high: 78, band: "high" });
  });

  it("stays on Reading when a complete version has no scores", () => {
    expect(libraryRow(summary("complete"), [run("greenline", null)])).toMatchObject({
      status: "reading",
      label: "Reading",
    });
  });
});

describe("drop copy", () => {
  it("follows empty, waiting, and the signed-in list", () => {
    expect(dropCopy([])).toEqual({
      title: "Drop a PDF or DOCX",
      hint: "Each system reads the file itself. Five megabytes.",
      tone: "empty",
    });
    expect(
      dropCopy([
        { id: "a", title: "ada-designed.pdf", filename: "ada-designed.pdf", overallScore: null, status: "waiting", label: "Stored. Waiting for the first read." },
      ]).title,
    ).toBe("Drop another file");
    expect(
      dropCopy([
        { id: "a", title: "ada-designed.pdf", filename: "ada-designed.pdf", overallScore: null, status: "reading", label: "Reading" },
      ]),
    ).toMatchObject({ title: "Drop a PDF or DOCX", tone: "list" });
  });
});

describe("teardown", () => {
  it("preserves backend summaries, advice, and each provider's independent extract", () => {
    const providerRun = { ...run("greenline", 35), parse: parse({ name: null, email: null }) };
    const cards = pluginCards(plugins, [providerRun]);
    expect(cards[0]).toMatchObject({
      id: "greenline",
      pending: false,
      summary: "Score 35 of 100.",
      parse: providerRun.parse,
      factors: [
        { label: "contact in header", delta: "-25", advice: "Move the email into the body." },
        { label: "standard headings", delta: "+10", advice: "Keep the standard headings." },
      ],
    });
    expect(cards[1]).toMatchObject({ id: "levr", pending: true });
  });

  const designed = parse({
    columns: 2,
    contact_in_header: true,
    has_tables: true,
  });

  it("shows completed callbacks while other providers are still reading the latest version", () => {
    const detail: ResumeDetail = {
      id: "resume-1",
      title: "ada-designed.pdf",
      created_at: "2026-11-07T18:00:00Z",
      versions: [
        version("running", designed, [run("greenline", 35)], "ada-designed.pdf", 2),
        version("complete", designed, [run("greenline", 95)], "old.pdf", 1),
      ],
    };
    const view = teardown(detail, plugins);
    expect(view).toMatchObject({
      kind: "reading",
      filename: "ada-designed.pdf",
      lede: readingLede(4),
    });
    if (view.kind !== "reading") throw new Error("expected reading");
    expect(view.extract?.map((field) => field.value)).toEqual([
      "Ada Lovelace",
      "ada@example.com",
      "2 columns, contact in the header, skills in a table",
      "Experience, Skills",
    ]);
    expect(view.cards[0]).toMatchObject({ pending: false, score: 35 });
    expect(view.cards.slice(1).every((card) => card.pending)).toBe(true);
    expect(view.parse).toEqual(designed);
    expect(view.cards.map((card) => card.name)).toEqual([
      "Greenline",
      "Levr",
      "Workdaye",
      "Ash",
    ]);
    expect(readingLede(4)).toBe(
      "Four systems are reading this file. They do not share a parse.",
    );
    expect(readingLede(1)).toBe(
      "One system is reading this file. It does not share a parse.",
    );
  });

  it("shows the extract and one card per plugin when the read is finished", () => {
    const detail: ResumeDetail = {
      id: "resume-1",
      title: "ada-designed.pdf",
      created_at: "2026-11-07T18:00:00Z",
      versions: [
        version(
          "complete",
          designed,
          [
            run("ash", 78),
            run("greenline", 35),
            run("levr", 70),
            run("stranger", 1),
          ],
          "ada-designed.pdf",
          1,
        ),
      ],
    };
    const view = teardown(detail, plugins);
    expect(view.kind).toBe("complete");
    if (view.kind !== "complete") throw new Error("expected complete");
    expect(view.file).toEqual({
      url: `/api/resumes/resume-1/versions/${detail.versions[0].id}/file`,
      mediaType: detail.versions[0].media_type,
    });
    expect(view.lede).toBe(
      "Contact in the header, skills in a table, two columns. Each system opened the file itself.",
    );
    expect(view.cards.map((card) => card.id)).toEqual([
      "greenline",
      "levr",
      "workdaye",
      "ash",
    ]);
    const greenline = view.cards[0];
    const missing = view.cards[2];
    const ash = view.cards[3];
    if (greenline.pending || ash.pending || !missing.pending) {
      throw new Error("expected greenline and ash scored, workdaye pending");
    }
    expect(greenline).toMatchObject({
      score: 35,
      band: "low",
      kept: ["Experience"],
      missed: ["table"],
      factors: [
        { label: "contact in header", delta: "-25" },
        { label: "standard headings", delta: "+10" },
      ],
    });
    expect(ash.score).toBe(78);
    expect(ash.band).toBe("high");
  });

  it("describes a plain file from the extract", () => {
    const plain = parse({ columns: 1, contact_in_header: false, has_tables: false });
    expect(completeLede(plain)).toBe(
      "One column. Contact in the body. Each system opened the file itself.",
    );
    expect(extractFields(plain).find((field) => field.label === "Layout")?.value).toBe(
      "1 column, contact in the body",
    );
  });

  it("leaves a blank when the extract has no name or sections", () => {
    expect(
      extractFields(parse({ name: null, email: "  ", sections: ["", "  "] })).map(
        (field) => field.value,
      ),
    ).toEqual(["\u2014", "\u2014", "1 column, contact in the body", "\u2014"]);
  });

  it("is missing when the resume has no versions", () => {
    expect(
      teardown(
        { id: "resume-1", title: "gone.pdf", created_at: "2026-11-07T18:00:00Z", versions: [] },
        plugins,
      ),
    ).toEqual({ kind: "missing" });
  });

  it("keeps a plugin pending when that run has no verdict", () => {
    const cards = pluginCards(plugins, [run("greenline", 95), run("levr", null)]);
    expect(cards[0].pending).toBe(false);
    expect(cards[1].pending).toBe(true);
    expect(cards[2].pending).toBe(true);
  });
});

describe("upload failures", () => {
  it("keeps a short API message and replaces a long one", () => {
    expect(uploadFailure(415, "file is neither a PDF nor a DOCX")).toBe(
      "file is neither a PDF nor a DOCX",
    );
    expect(uploadFailure(413, "")).toBe("That file is over five megabytes.");
    expect(uploadFailure(500, "x".repeat(201))).toBe("That file did not store.");
  });
});

describe("version selection", () => {
  const detail: ResumeDetail = {
    id: "resume-1", title: "ada.pdf", created_at: "2026-11-07T18:00:00Z",
    versions: [
      version("running", null, [run("greenline", null)], "new.pdf", 2),
      version("complete", parse({ name: "Old name" }), [run("greenline", 95)], "old.pdf", 1),
    ],
  };

  it("defaults to latest and lists the available versions", () => {
    expect(teardown(detail, plugins)).toMatchObject({
      kind: "reading", title: "ada.pdf", versionId: "version-2", filename: "new.pdf",
      versions: [{ id: "version-2", number: 2 }, { id: "version-1", number: 1 }],
    });
  });

  it("uses only the selected version's parse, status, and verdicts", () => {
    const view = teardown(detail, plugins, "version-1");
    expect(view).toMatchObject({ kind: "complete", title: "ada.pdf", versionId: "version-1", filename: "old.pdf" });
    if (view.kind === "missing") throw new Error("expected version");
    expect(view.extract?.[0].value).toBe("Old name");
    expect(view.file.url).toBe("/api/resumes/resume-1/versions/version-1/file");
    expect(view.cards[0]).toMatchObject({ pending: false, score: 95 });
  });

  it("carries the app's section reading for the selected version", () => {
    const map = {
      engine: "fastembed",
      model: "bge-small",
      sections: [{
        id: "experience-1",
        kind: "experience" as const,
        heading: "Experience",
        body: "Built things.",
        confidence: 0.94,
        feedback: "up" as const,
      }],
    };
    const withSections = {
      ...version("complete", parse({}), [], "old.pdf", 1),
      sections: map,
    };
    expect(teardown({ ...detail, versions: [withSections] }, plugins, "version-1"))
      .toMatchObject({ sections: map });
  });

  it("carries the generated profile and its vote for the selected version", () => {
    const profileSummary = {
      text: "Ada is a computing pioneer.",
      engine: "ollama",
      model: "qwen2.5:0.5b",
      is_resume: true,
      feedback: "up" as const,
    };
    const withSummary = {
      ...version("complete", parse({}), [], "old.pdf", 1),
      profile_summary: profileSummary,
    };
    expect(teardown({ ...detail, versions: [withSummary] }, plugins, "version-1"))
      .toMatchObject({ profileSummary });
  });

  it("does not silently show the latest results for a missing version", () => {
    expect(teardown(detail, plugins, "deleted-version")).toEqual({ kind: "missing" });
  });
});

describe("version API requests", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("fetches fresh detail and returns the requested version's results", async () => {
    const fetch = vi.fn(async (url: URL) => Response.json(url.pathname === "/api/plugins"
      ? { plugins }
      : { id: "resume-1", title: "ada.pdf", created_at: "2026-11-07T18:00:00Z", versions: [
        version("running", null, [], "new.pdf", 2),
        version("complete", parse({}), [run("greenline", 95)], "old.pdf", 1),
      ] }));
    vi.stubGlobal("fetch", fetch);
    expect(await fetchTeardown("http://api.test", "resume-1", "session=test", "version-1"))
      .toMatchObject({ filename: "old.pdf", cards: expect.arrayContaining([expect.objectContaining({ score: 95 })]) });
    expect(fetch).toHaveBeenCalledWith(new URL("http://api.test/api/resumes/resume-1"),
      expect.objectContaining({ headers: expect.any(Headers) }));
  });

  it("uploads revisions to the existing resume with the session cookie", async () => {
    const fetch = vi.fn(async () => Response.json({ id: "resume-1", version_id: "version-2" }, { status: 202 }));
    vi.stubGlobal("fetch", fetch);
    const file = new File(["%PDF"], "revision.pdf", { type: "application/pdf" });
    expect(await postResume("http://api.test", file, "session=test", "resume-1"))
      .toEqual({ ok: true, id: "resume-1", version_id: "version-2" });
    const [url, init] = fetch.mock.calls[0] as unknown as [URL, RequestInit];
    expect(url.pathname).toBe("/api/resumes/resume-1/versions");
    expect(init.method).toBe("POST");
    expect((init.headers as Headers).get("cookie")).toBe("session=test");
    expect((init.body as FormData).get("file")).toBeInstanceOf(File);
  });

  it("targets one version or the entire resume and reports failures", async () => {
    const fetch = vi.fn(async () => new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetch);
    expect(await deleteResume("http://api.test", "resume-1", "session=test", "version-1")).toEqual({ ok: true });
    expect(await deleteResume("http://api.test", "resume-1", "session=test")).toEqual({ ok: true });
    const calls = fetch.mock.calls as unknown as [URL, RequestInit][];
    expect(calls.map(([url]) => url.pathname)).toEqual([
      "/api/resumes/resume-1/versions/version-1", "/api/resumes/resume-1",
    ]);
    expect(calls.every(([, init]) => init.method === "DELETE")).toBe(true);
    fetch.mockImplementation(async () => new Response(null, { status: 500 }));
    expect(await deleteResume("http://api.test", "resume-1", undefined)).toMatchObject({ ok: false });
  });

  it("records and clears a vote on one detected section", async () => {
    const fetch = vi.fn(async () => Response.json({ vote: "up" }));
    vi.stubGlobal("fetch", fetch);
    expect(
      await voteSection("http://api.test", "resume-1", "version-1", "experience-1", "up", "session=test"),
    ).toEqual({ ok: true, vote: "up" });
    const [url, init] = fetch.mock.calls[0] as unknown as [URL, RequestInit];
    expect(url.pathname).toBe("/api/resumes/resume-1/versions/version-1/sections/experience-1");
    expect(init.method).toBe("PUT");
    expect(new Headers(init.headers).get("cookie")).toBe("session=test");
    expect(JSON.parse(String(init.body))).toEqual({ vote: "up" });

    fetch.mockImplementation(async () => new Response(null, { status: 404 }));
    expect(
      await voteSection("http://api.test", "resume-1", "version-1", "missing-1", null, undefined),
    ).toMatchObject({ ok: false });
  });
});
