import { Link } from "@tanstack/react-router";

import type { LibraryRow } from "~/server/resumes";
import { letterGrade, ScoreStamp, scoreTone } from "./score";
import { CheckCheck, Clock3, LoaderCircle } from "lucide-react";
import { DeleteResume } from "./delete-resume";

export function ResumeRow({ row }: { row: LibraryRow }) {
  return (
    <div className="library-row sheet-face">
      <Link to="/resumes/$id" params={{ id: row.id }} className="file file-row">
        <div className="row-identity">
          <b>{row.title}</b>
          {row.title !== row.filename ? <span className="set-filename">{row.filename}</span> : null}
          <span className={`row-analysis-status ${row.status === "scored" ? "is-complete" : ""}`}>
            {row.status === "scored" ? <CheckCheck size={14} aria-hidden="true" /> : row.status === "waiting" ? <Clock3 size={14} aria-hidden="true" /> : <LoaderCircle size={14} className="busy-icon" aria-hidden="true" />}
            {row.status === "scored" ? `Analysis complete · ${row.readsLabel}` : row.status === "waiting" ? "Waiting for analysis" : "Analysis in progress"}
          </span>
        </div>
        <div className="row-overall" aria-label="Overall rating">
          <div><span className="row-grade-label">Overall grade{row.status === "scored" ? "" : " · Provisional"}</span><strong className={row.overallScore === null ? "" : scoreTone(row.overallScore)}>{row.overallScore === null ? "—" : letterGrade(row.overallScore)}</strong></div>
          {row.overallScore !== null ? <ScoreStamp value={row.overallScore} /> : null}
        </div>
      </Link>
      <DeleteResume id={row.id} filename={row.title} />
    </div>
  );
}
