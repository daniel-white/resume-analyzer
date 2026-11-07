import { isRedirect, useNavigate, useRouter } from "@tanstack/react-router";
import { useId, useState, type ChangeEvent } from "react";

import { removeResume, uploadResume } from "~/fns/resumes.functions";
import type { VersionSummary } from "~/server/resumes";

export function ResumeVersions({ resumeId, versionId, versions }: {
  resumeId: string;
  versionId: string;
  versions: VersionSummary[];
}) {
  const navigate = useNavigate();
  const router = useRouter();
  const inputId = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"version" | "all" | null>(null);
  const selected = versions.find((version) => version.id === versionId)!;

  async function handleError(error: unknown, message: string) {
    if (isRedirect(error)) await navigate({ to: "/login" });
    else setError(message);
  }

  async function selectVersion(id: string) {
    setBusy(true);
    setError(null);
    setConfirm(null);
    try {
      await navigate({ to: "/resumes/$id", params: { id: resumeId }, search: { version: id } });
    } catch (error) {
      await handleError(error, "Could not load that version. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  async function upload(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || busy) return;
    setBusy(true);
    setError(null);
    setConfirm(null);
    try {
      const data = new FormData();
      data.set("file", file, file.name);
      data.set("resumeId", resumeId);
      const result = await uploadResume({ data });
      if (!result.ok) { setError(result.message); return; }
      await navigate({ to: "/resumes/$id", params: { id: resumeId }, search: { version: result.version_id } });
    } catch (error) {
      await handleError(error, "That file did not store. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!confirm || busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await removeResume({ data: {
        id: resumeId, versionId: confirm === "version" ? versionId : undefined,
      } });
      if (!result.ok) { setError(result.message); return; }
      setConfirm(null);
      if (confirm === "all" || versions.length === 1) {
        await navigate({ to: "/" });
      } else {
        const remaining = versions.find((version) => version.id !== versionId)!;
        await navigate({ to: "/resumes/$id", params: { id: resumeId }, search: { version: remaining.id }, replace: true });
      }
      await router.invalidate();
    } catch (error) {
      await handleError(error, "Could not delete the resume. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="sheet-face resume-versions" aria-label="Resume versions" aria-busy={busy}>
      <div className="version-actions">
        <label className="version-picker">
          <span>Version</span>
          <select aria-label="Version" value={versionId} disabled={busy} onChange={(event) => void selectVersion(event.target.value)}>
            {versions.map((version, index) => (
              <option key={version.id} value={version.id}>
                v{version.number}{index === 0 ? " · Latest" : ""} — {version.filename}
              </option>
            ))}
          </select>
        </label>
        <label className="version-upload" htmlFor={inputId}>
          Upload new version
          <input id={inputId} type="file" aria-label="Upload new version"
            accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            disabled={busy} onChange={upload} />
        </label>
        <button type="button" disabled={busy} onClick={() => setConfirm("version")}>Delete this version</button>
        <button type="button" disabled={busy} onClick={() => setConfirm("all")}>Delete all versions</button>
      </div>
      <p className="version-note">{versions.length} {versions.length === 1 ? "version" : "versions"} stored. PDF or DOCX, up to five megabytes.</p>
      {busy ? <p role="status">Working…</p> : null}
      {confirm ? (
        <div className="version-confirm" role="alertdialog" aria-label="Confirm deletion" aria-describedby={`${inputId}-confirmation`}>
          <p id={`${inputId}-confirmation`}>{confirm === "all"
            ? "Delete this resume, all its versions, and their results? This cannot be undone."
            : `Delete v${selected.number} (${selected.filename}) and its results? ${versions.length === 1 ? "This is the last version, so the resume will also be removed. " : ""}This cannot be undone.`}</p>
          <button type="button" disabled={busy} onClick={() => void remove()}>Confirm deletion</button>
          <button type="button" disabled={busy} onClick={() => setConfirm(null)}>Cancel</button>
        </div>
      ) : null}
      {error ? <p className="error" role="alert">{error}</p> : null}
    </section>
  );
}
