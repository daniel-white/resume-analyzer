import { Link } from "@tanstack/react-router";

import type { LibraryRow } from "~/server/resumes";

export function ResumeRow({ row }: { row: LibraryRow }) {
  return (
    <Link to="/resumes/$id" params={{ id: row.id }} className="file">
      <b>{row.filename}</b>
      {row.status === "scored" ? (
        <span>
          {row.readsLabel} <span className={`spread ${row.band}`}>{row.spread}</span>
        </span>
      ) : (
        <span>{row.label}</span>
      )}
    </Link>
  );
}
