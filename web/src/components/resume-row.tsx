import { Link } from "@tanstack/react-router";

import type { LibraryRow } from "~/server/resumes";
import { letterGrade, scoreTone } from "./score";

export function ResumeRow({ row }: { row: LibraryRow }) {
  return (
    <Link to="/resumes/$id" params={{ id: row.id }} className="file file-row sheet-face">
      <b>{row.filename}</b>
      {row.status === "scored" ? (
        <span>
          {row.readsLabel} <span className={scoreTone(row.low)} title={row.spread}>
            {letterGrade(row.low) === letterGrade(row.high)
              ? letterGrade(row.low)
              : `${letterGrade(row.low)}–${letterGrade(row.high)}`}
          </span>
        </span>
      ) : (
        <span>{row.label}</span>
      )}
    </Link>
  );
}
