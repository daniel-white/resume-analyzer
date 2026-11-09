import { FactorList } from "~/components/factor-list";
import { Score, ScoreStamp } from "~/components/score";
import { Sheet } from "~/components/sheet";
import { headingMatches, providerFound, sectionKindLabel } from "~/lib/sections";
import type { ParseInput, SectionKind, Verdict } from "~/server/resumes";
import { cn } from "~/lib/utils";
import { LoaderCircle } from "lucide-react";

type CardVerdict = Pick<Verdict, "score" | "kept" | "dropped" | "factors"> & {
  summary?: string;
  report?: Verdict["report"];
};

const marked = new Set(["greenline", "levr", "workdaye", "ash", "smart_resume", "indy_hackers_hiring", "blast_track"]);

const aside: Record<string, string> = {
  greenline: "Skips the header. Likes plain headings.",
  levr: "Counts a skill only when the words match.",
  workdaye: "Wants your email in the body, and dates written as MM/YYYY.",
  ash: "Keeps more of the page. Drops pictures.",
};

export function PluginCard({
  plugin,
  verdict,
  reading,
  selected,
  onSelect,
  parse,
  highlightKind,
}: {
  plugin: { id: string; name: string; bias: string };
  verdict: CardVerdict | null;
  reading: boolean;
  selected: boolean;
  onSelect: () => void;
  parse?: ParseInput | null;
  highlightKind?: SectionKind | null;
}) {
  const highlighted = highlightKind && verdict
    ? providerFound(highlightKind, { parse, kept: verdict.kept })
    : null;
  return (
    <Sheet>
      <details className={cn("sheet-face", selected && "sheet-selected")} open={selected}>
        <summary
          className="fold-summary"
          onClick={(event) => {
            event.preventDefault();
            onSelect();
          }}
        >
          {marked.has(plugin.id) ? (
            <span className="mark-frame">
              <img src={`/marks/${plugin.id}.png`} alt="" className="mark-img" />
            </span>
          ) : null}
          <span className="min-w-0">
            <h2 className="block text-lg font-medium">{plugin.name}</h2>
            <span className="block text-xs text-muted-foreground">{plugin.bias}</span>
            {reading ? <span className="pending-read" aria-label="Pending"><LoaderCircle className="busy-icon" size={13} aria-hidden="true" />Reading the file</span> : null}
          </span>
          {reading || verdict === null ? null : (
            <span className="read-marks">
              <Score value={verdict.score} compact />
              <ScoreStamp value={verdict.score} />
            </span>
          )}
          <span className="fold-chevron" aria-hidden />
        </summary>
        {reading || verdict === null ? (
          <div className="fold-body">
            <p className="text-muted-foreground">Still reading your file.</p>
          </div>
        ) : (
          <div className="fold-body">
            <p>{verdict.summary ?? aside[plugin.id] ?? plugin.bias}</p>
            {highlightKind && highlighted !== null ? (
              <p className={cn("section-verdict", highlighted ? "is-found" : "is-missed")}>
                {highlighted
                  ? `Found the ${sectionKindLabel(highlightKind)} section`
                  : `Did not find the ${sectionKindLabel(highlightKind)} section`}
              </p>
            ) : null}
            <div className="mt-4">
              <h3 className="text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">
                Kept
              </h3>
              <ul className="mt-1.5 flex flex-col gap-1">
                {(verdict.kept.length === 0 ? ["Nothing"] : verdict.kept).map((line) => (
                  <li key={line} className={cn(highlightKind && headingMatches(highlightKind, line) && "extract-hit")}>{line}</li>
                ))}
              </ul>
            </div>
            <div className="mt-4">
              <h3 className="text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">
                Missed
              </h3>
              <ul className="mt-1.5 flex flex-col gap-1">
                {(verdict.dropped.length === 0 ? ["Nothing"] : verdict.dropped).map((line) => (
                  <li key={line}>{line}</li>
                ))}
              </ul>
            </div>
            <h3 className="mt-4 text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">
              Why
            </h3>
            <FactorList factors={verdict.factors} />
            {verdict.report ? <ProviderReportView report={verdict.report} /> : null}
            {parse ? (
              <section className="mt-4 border-t pt-3" aria-label={`${plugin.name} extract`}>
                <h3 className="text-xs font-medium tracking-[0.08em] uppercase text-muted-foreground">This system's extract</h3>
                <p className="mt-2 text-sm">{[parse.profile.name, parse.profile.email, parse.profile.phone].filter(Boolean).join(" · ") || "No contact details extracted."}</p>
                {parse.profile.sections.map((section, index) => (
                  <div
                    className={cn("mt-3 text-sm", highlightKind && headingMatches(highlightKind, section.heading) && "extract-hit")}
                    key={`${section.heading}-${index}`}
                  >
                    <h4 className="font-medium">{section.heading}</h4>
                    <p className="whitespace-pre-wrap">{section.body}</p>
                  </div>
                ))}
              </section>
            ) : null}
          </div>
        )}
      </details>
    </Sheet>
  );
}

export function ProviderReportView({ report }: { report: NonNullable<Verdict["report"]> }) {
  const outcome = {
    parsed: "Parsed",
    parsed_with_warnings: "Parsed with warnings",
    unreadable: "Could not read file",
  }[report.outcome];
  return (
    <section className="mt-4 border-t pt-3" aria-label="Simulated parser report">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-xs font-medium tracking-[0.08em] uppercase text-muted-foreground">Simulated parser report</h3>
        <span className="text-xs font-medium">{outcome}</span>
      </div>
      <p className="mt-2 text-sm">{report.engine} <span className="font-mono text-xs text-muted-foreground">v{report.engine_version}</span></p>
      <p className="mt-1 break-all font-mono text-xs text-muted-foreground">Report {report.reference}</p>
      {report.outcome !== "unreadable" ? (
        <dl className="mt-3 grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
          <div><dt className="text-muted-foreground">Contact fields detected</dt><dd className="mt-0.5 font-mono">{report.contact_fields} / 3</dd></div>
          <div><dt className="text-muted-foreground">Section headings detected</dt><dd className="mt-0.5 font-mono">{report.sections_detected}</dd></div>
          <div><dt className="text-muted-foreground">Bullets detected</dt><dd className="mt-0.5 font-mono">{report.bullets_detected}</dd></div>
          <div><dt className="text-muted-foreground">Layout evidence</dt><dd className="mt-0.5">{report.layout_confidence === "structural" ? "DOCX markup" : "PDF inference"}</dd></div>
        </dl>
      ) : null}
      <ul className="mt-3 flex flex-col gap-1.5 text-xs text-muted-foreground">
        {report.notes.map((note, index) => <li key={index}>{note}</li>)}
      </ul>
      <p className="mt-3 text-xs text-muted-foreground">Fictional engine metadata. Detection counts are not a guarantee that fields survived scoring.</p>
    </section>
  );
}
