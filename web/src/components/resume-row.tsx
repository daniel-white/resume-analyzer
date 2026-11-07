import { Link } from "@tanstack/react-router";

import type { LibraryRow } from "~/server/resumes";
import { letterGrade, scoreTone } from "./score";
import { DeleteResume } from "./delete-resume";

export function ResumeRow({ row }: { row: LibraryRow }) {
  return (
    <div className="library-row sheet-face">
      <Link to="/resumes/$id" params={{ id: row.id }} className="file file-row">
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
      <DeleteResume id={row.id} filename={row.filename} />
    </div>
  );
}
