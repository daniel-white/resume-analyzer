import { isRedirect, useNavigate } from "@tanstack/react-router";
import { useRef, useState, type ChangeEvent, type DragEvent } from "react";
import { FileText, LoaderCircle, Upload } from "lucide-react";

import { uploadResume } from "~/fns/resumes.functions";
import type { DropCopy } from "~/server/resumes";

export function DropZone({ copy }: { copy: DropCopy }) {
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const dragDepth = useRef(0);
  const tone = copy.tone === "empty" ? "" : copy.tone;

  async function onChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    await store(file);
  }

  async function store(file?: File) {
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

  function onDrop(event: DragEvent<HTMLLabelElement>) {
    event.preventDefault();
    dragDepth.current = 0;
    setDragging(false);
    if (event.dataTransfer.files.length > 1) {
      setError("Please upload one resume at a time.");
      return;
    }
    void store(event.dataTransfer.files[0]);
  }

  return (
    <label className={`sheet-face drop ${tone} ${dragging ? "is-dragging" : ""}`} aria-busy={busy}
      onDragEnter={(event) => { event.preventDefault(); dragDepth.current++; if (!busy) setDragging(true); }}
      onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = busy ? "none" : "copy"; }}
      onDragLeave={(event) => { event.preventDefault(); dragDepth.current--; if (dragDepth.current <= 0) setDragging(false); }}
      onDrop={onDrop}>
      <input
        type="file"
        aria-label="Upload resume"
        name="file"
        accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        onChange={onChange}
        disabled={busy}
      />
      <span className={`drop-icon ${copy.tone === "empty" ? "first-upload" : ""}`} aria-hidden="true">{busy ? <LoaderCircle className="busy-icon" size={30} /> : <FileText size={30} strokeWidth={1.4} />}</span>
      <h2>{busy ? "Storing the file" : copy.title}</h2>
      {copy.hint ? <p>{copy.hint}</p> : null}
      <span className="upload-callout">{busy ? "Starting your independent reads…" : <><Upload size={16} aria-hidden="true" />Choose a resume</>}</span>
      <span className="upload-formats">PDF or DOCX · Up to 5 MB</span>
      {busy ? <span className="sr-only" role="status">Storing your resume. Please wait.</span> : null}
      {error ? (
        <p className="error" role="alert">
          {error}
        </p>
      ) : null}
    </label>
  );
}
