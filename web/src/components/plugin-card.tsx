import { FactorList } from "~/components/factor-list";
import { Score, ScoreStamp } from "~/components/score";
import { Sheet } from "~/components/sheet";
import type { ParseInput, Verdict } from "~/server/resumes";
import { cn } from "~/lib/utils";
import { LoaderCircle } from "lucide-react";

type CardVerdict = Pick<Verdict, "score" | "kept" | "dropped" | "factors"> & {
  summary?: string;
};

const marked = new Set(["greenline", "levr", "workdaye", "ash", "smart_resume"]);

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
}: {
  plugin: { id: string; name: string; bias: string };
  verdict: CardVerdict | null;
  reading: boolean;
  selected: boolean;
  onSelect: () => void;
  parse?: ParseInput | null;
}) {
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
            <div className="mt-4">
              <h3 className="text-xs font-medium tracking-[0.08em] text-muted-foreground uppercase">
                Kept
              </h3>
              <ul className="mt-1.5 flex flex-col gap-1">
                {(verdict.kept.length === 0 ? ["Nothing"] : verdict.kept).map((line) => (
                  <li key={line}>{line}</li>
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
            {parse ? (
              <section className="mt-4 border-t pt-3" aria-label={`${plugin.name} extract`}>
                <h3 className="text-xs uppercase text-muted-foreground">This system's extract</h3>
                <p className="mt-2 text-sm">{[parse.profile.name, parse.profile.email, parse.profile.phone].filter(Boolean).join(" · ") || "No contact details extracted."}</p>
                {parse.profile.sections.map((section, index) => (
                  <div className="mt-3 text-sm" key={`${section.heading}-${index}`}>
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
