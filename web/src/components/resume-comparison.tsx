import { ArrowDown, ArrowRight, ArrowUp, ChevronDown, GitCompareArrows, Minus, ScanText } from "lucide-react";
import { useState } from "react";

import { NATURE_LABELS } from "~/lib/skills";
import { compareVersions, comparisonMessage } from "~/lib/version-comparison";
import type { ComparisonVersion, SkillSummary } from "~/server/resumes";
import { letterGrade, scoreTone } from "./score";

const marked = new Set(["greenline", "levr", "workdaye", "ash", "smart_resume", "indy_hackers_hiring", "blast_track"]);

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

function SkillChip({ skill, suffix }: { skill: SkillSummary; suffix?: string }) {
  return <span
    className={`skill-change-chip skill-change-chip--${skill.nature}`}
    data-testid="skill-change"
    title={`${NATURE_LABELS[skill.nature]} skill`}
  >
    {skill.name}
    {suffix ? <span className="skill-change-delta">{suffix}</span> : null}
  </span>;
}

export function ResumeComparison({ versions, versionId }: {
  versions: ComparisonVersion[];
  versionId: string;
}) {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const ordered = [...versions].sort((a, b) => a.number - b.number);
  const first = ordered[0];
  const last = ordered.at(-1);

  const disclosure = <summary className="comparison-disclosure">
    <GitCompareArrows size={16} aria-hidden="true" /><span>Compare versions</span>
    <span className="comparison-disclosure-hint">See score changes over time</span>
    <ChevronDown className="comparison-chevron" size={16} aria-hidden="true" />
  </summary>;

  const empty = <section className="sheet-face resume-comparison" aria-label="Compare versions">
    <details>{disclosure}<p className="comparison-empty">Upload a new version to see how your scores change. Your first version is a great starting point.</p></details>
  </section>;

  if (!first || !last) return empty;

  const selected = ordered.find((version) => version.id === (toId || versionId)) ?? last;
  const to = selected.number === first.number && ordered.length > 1 ? ordered[1] ?? last : selected;
  const from = ordered.find((version) => version.id === fromId && version.number < to.number)
    ?? ordered.filter((version) => version.number < to.number).at(-1);

  if (!from) return empty;

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
    {(comparison.skills.added.length > 0
      || comparison.skills.removed.length > 0
      || comparison.skills.moved.length > 0) ? (
      <div className="comparison-skills" aria-label="Skill changes between versions">
        <p className="comparison-skills-heading">Skill signal changes</p>
        {comparison.skills.added.length > 0 ? (
          <p className="comparison-skill-group">
            <span className="comparison-skill-label">Gained</span>
            {comparison.skills.added.map((skill) => <SkillChip key={skill.canonical} skill={skill} />)}
          </p>
        ) : null}
        {comparison.skills.moved.length > 0 ? (
          <p className="comparison-skill-group">
            <span className="comparison-skill-label">Changed importance</span>
            {comparison.skills.moved.map((skill) => (
              <SkillChip
                key={skill.canonical}
                skill={skill}
                suffix={`${skill.delta > 0 ? "+" : ""}${skill.delta}`}
              />
            ))}
          </p>
        ) : null}
        {comparison.skills.removed.length > 0 ? (
          <p className="comparison-skill-group">
            <span className="comparison-skill-label">No longer detected</span>
            {comparison.skills.removed.map((skill) => <SkillChip key={skill.canonical} skill={skill} />)}
          </p>
        ) : null}
        <p className="comparison-note">Importance is measured from evidence in each résumé, not from a job description.</p>
      </div>
    ) : null}
    <p className="comparison-note">Simulated ATS scores are feedback on parsing, not a measure of your potential or a prediction of hiring outcomes.</p>
    </details>
  </section>;
}
