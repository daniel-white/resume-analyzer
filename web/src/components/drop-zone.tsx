import { isRedirect, useNavigate } from "@tanstack/react-router";
import { useState, type ChangeEvent } from "react";

import { uploadResume } from "~/fns/resumes.functions";
import type { DropCopy } from "~/server/resumes";

export function DropZone({ copy }: { copy: DropCopy }) {
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const tone = copy.tone === "empty" ? "" : copy.tone;

  async function onChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || busy) return;
    const body = new FormData();
    body.set("file", file, file.name);
    setBusy(true);
    setError(null);
    try {
      const result = await uploadResume({ data: body });
      if (!result.ok) {
        setError(result.message);
        setBusy(false);
        return;
      }
      await navigate({ to: "/resumes/$id", params: { id: result.id } });
    } catch (error) {
      if (isRedirect(error)) {
        await navigate({ to: "/login" });
        return;
      }
      setError("That file did not store.");
      setBusy(false);
    }
  }

  return (
    <label className={`sheet-face drop ${tone}`} aria-busy={busy}>
      <input
        type="file"
        name="file"
        accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        onChange={onChange}
        disabled={busy}
      />
      <h1>{busy ? "Storing the file" : copy.title}</h1>
      {copy.hint ? <p>{copy.hint}</p> : null}
      {error ? (
        <p className="error" role="alert">
          {error}
        </p>
      ) : null}
    </label>
  );
}
