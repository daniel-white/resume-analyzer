import { describe, expect, it } from "vitest";

import {
  completeLede,
  dropCopy,
  extractFields,
  libraryRow,
  pluginCards,
  readingLede,
  scoreBand,
  teardown,
  uploadFailure,
  type ParseInput,
  type Plugin,
  type ResumeDetail,
  type ResumeStatus,
  type ResumeSummary,
  type RunView,
  type Verdict,
} from "./resumes";

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
  it("says a stored file is waiting on the first read", () => {
    expect(libraryRow(summary("uploaded"), [])).toEqual({
      id: "resume-1",
      filename: "ada-designed.pdf",
      status: "waiting",
      label: "Stored. Waiting for the first read.",
    });
  });

  it("says parsed and running files are still reading", () => {
    expect(libraryRow(summary("parsed"), [run("greenline", null)]).status).toBe("reading");
    const running = libraryRow(summary("running"), [run("greenline", 35)]);
    expect(running.status === "reading" && running.label).toBe("Reading");
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
        { id: "a", filename: "ada-designed.pdf", status: "waiting", label: "Stored. Waiting for the first read." },
      ]).title,
    ).toBe("Drop another file");
    expect(
      dropCopy([
        { id: "a", filename: "ada-designed.pdf", status: "reading", label: "Reading" },
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
