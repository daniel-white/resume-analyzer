import { ArrowDown, ArrowRight, ArrowUp, ChevronDown, GitCompareArrows, Minus, ScanText } from "lucide-react";
import { useState } from "react";

import { compareVersions, comparisonMessage } from "~/lib/version-comparison";
import type { ComparisonVersion } from "~/server/resumes";
import { letterGrade, scoreTone } from "./score";

const marked = new Set(["greenline", "levr", "workdaye", "ash", "smart_resume"]);

function ComparisonScore({ value }: { value: number | null }) {
  if (value === null) return <span className="text-muted-foreground">Pending</span>;
  return <span className="comparison-score-value">
    <span className={`comparison-grade ${scoreTone(value)}`} aria-label={`Grade ${letterGrade(value)}`}>{letterGrade(value)}</span>
    <span className="comparison-numeric-score">{value}</span>
  </span>;
}

function ScoreChange({ delta }: { delta: number | null }) {
  if (delta === null) return <span className="score-change">Awaiting results</span>;
  const Icon = delta > 0 ? ArrowUp : delta < 0 ? ArrowDown : Minus;
  return <span className={`score-change ${delta > 0 ? "up" : delta < 0 ? "down" : "unchanged"}`}>
    <Icon size={16} aria-hidden="true" />
    {delta === 0 ? "Unchanged" : `${delta > 0 ? "Up" : "Down"} ${Math.abs(delta)} ${Math.abs(delta) === 1 ? "point" : "points"}`}
  </span>;
}

export function ResumeComparison({ versions, versionId }: {
  versions: ComparisonVersion[];
  versionId: string;
}) {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const ordered = [...versions].sort((a, b) => a.number - b.number);
  const selected = ordered.find((version) => version.id === (toId || versionId)) ?? ordered[ordered.length - 1];
  const to = selected.number === ordered[0].number && ordered.length > 1 ? ordered[1] : selected;
  const from = ordered.find((version) => version.id === fromId && version.number < to.number)
    ?? ordered.filter((version) => version.number < to.number).at(-1);

  const disclosure = <summary className="comparison-disclosure">
    <GitCompareArrows size={16} aria-hidden="true" /><span>Compare versions</span>
    <span className="comparison-disclosure-hint">See score changes over time</span>
    <ChevronDown className="comparison-chevron" size={16} aria-hidden="true" />
  </summary>;

  if (!from) return <section className="sheet-face resume-comparison" aria-label="Compare versions">
    <details>{disclosure}<p className="comparison-empty">Upload a new version to see how your scores change. Your first version is a great starting point.</p></details>
  </section>;

  const comparison = compareVersions(from, to);
  return <section className="sheet-face resume-comparison" aria-label="Compare versions">
    <details>{disclosure}
    <div className="comparison-heading"><p>Compare an earlier version with a later revision. Changes always run forward in time.</p></div>
    <div className="comparison-pickers">
      <label className="version-picker"><span className="version-label">From version</span>
        <select className="polished-select" value={from.id} onChange={(event) => {
          const next = ordered.find((version) => version.id === event.target.value)!;
          setFromId(next.id);
          if (next.number >= to.number) setToId(ordered.find((version) => version.number > next.number)!.id);
        }}>
          {ordered.slice(0, -1).map((version) => <option key={version.id} value={version.id}>v{version.number} — {version.filename}</option>)}
        </select>
      </label>
      <ArrowRight className="comparison-direction" size={18} aria-hidden="true" />
      <label className="version-picker"><span className="version-label">To version</span>
        <select className="polished-select" value={to.id} onChange={(event) => setToId(event.target.value)}>
          {ordered.filter((version) => version.number > from.number).map((version) => <option key={version.id} value={version.id}>v{version.number} — {version.filename}</option>)}
        </select>
      </label>
    </div>
    <div className="comparison-summary" role="status" aria-live="polite" aria-atomic="true">
      <div className="comparison-score"><span>Overall score (median)</span>
        <strong><ComparisonScore value={comparison.before} /> <ArrowRight size={18} aria-label="to" /> <ComparisonScore value={comparison.after} /></strong>
        <ScoreChange delta={comparison.delta} />
      </div>
      <div><p>{comparisonMessage(comparison.delta)}</p>
        <p className="comparison-counts">{comparison.up} up · {comparison.down} down · {comparison.unchanged} unchanged</p>
        <p className="comparison-note">Based on {comparison.matched} {comparison.matched === 1 ? "system" : "systems"} with scores in both versions. Missing results are not counted as zero.</p>
      </div>
    </div>
    <div className="comparison-table-scroll"><table className="comparison-table">
      <caption>ATS score changes from v{from.number} to v{to.number}</caption>
      <thead><tr><th scope="col">System</th><th scope="col">v{from.number}</th><th scope="col">v{to.number}</th><th scope="col">Change</th></tr></thead>
      <tbody>{comparison.rows.map((row) => <tr key={row.id}>
        <th scope="row"><span className="comparison-system">
          <span className="mark-frame comparison-system-mark" aria-hidden="true">
            {marked.has(row.id) ? <img src={`/marks/${row.id}.png`} alt="" className="mark-img" /> : <ScanText size={20} />}
          </span>
          <span>{row.name}</span>
        </span></th>
        <td><ComparisonScore value={row.before} /></td><td><ComparisonScore value={row.after} /></td><td><ScoreChange delta={row.delta} /></td>
      </tr>)}</tbody>
    </table></div>
    <p className="comparison-note">Simulated ATS scores are feedback on parsing, not a measure of your potential or a prediction of hiring outcomes.</p>
    </details>
  </section>;
}
