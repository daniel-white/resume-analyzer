import { isRedirect, useNavigate, useRouter } from "@tanstack/react-router";
import { useId, useRef, useState, type ChangeEvent } from "react";
import { AlertDialog, DropdownMenu } from "radix-ui";
import { AlertTriangle, CheckCircle2, ChevronDown, FileClock, LoaderCircle, Trash2, Upload, X } from "lucide-react";

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
  const [busy, setBusy] = useState<"select" | "upload" | "delete" | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"version" | "all" | null>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const openerRef = useRef<HTMLButtonElement | null>(null);
  const menuTriggerRef = useRef<HTMLButtonElement>(null);
  const openingConfirmation = useRef(false);
  const selected = versions.find((version) => version.id === versionId)!;

  async function handleError(error: unknown, message: string) {
    if (isRedirect(error)) await navigate({ to: "/login" });
    else setError(message);
  }

  async function selectVersion(id: string) {
    setBusy("select");
    setNotice(null);
    setError(null);
    setConfirm(null);
    try {
      await navigate({ to: "/resumes/$id", params: { id: resumeId }, search: { version: id } });
    } catch (error) {
      await handleError(error, "Could not load that version. Please try again.");
    } finally {
      setBusy(null);
    }
  }

  async function upload(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || busy) return;
    setBusy("upload");
    setNotice(null);
    setError(null);
    setConfirm(null);
    try {
      const data = new FormData();
      data.set("file", file, file.name);
      data.set("resumeId", resumeId);
      const result = await uploadResume({ data });
      if (!result.ok) { setError(result.message); return; }
      await navigate({ to: "/resumes/$id", params: { id: resumeId }, search: { version: result.version_id } });
      setNotice("New version uploaded. Previous versions are unchanged.");
    } catch (error) {
      await handleError(error, "Upload failed. Please try again.");
    } finally {
      setBusy(null);
    }
  }

  async function remove() {
    if (!confirm || busy) return;
    setBusy("delete");
    setNotice(null);
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
        setNotice(`Version ${selected.number} deleted.`);
      }
      await router.invalidate();
    } catch (error) {
      await handleError(error, "Could not delete the résumé. Please try again.");
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="sheet-face resume-versions" aria-label="Résumé versions" aria-busy={!!busy}>
      <div className="version-actions">
        <label className="version-picker">
          <span className="version-label"><FileClock size={14} aria-hidden="true" /> Version history</span>
          <select className="polished-select" aria-label="Version" value={versionId} disabled={!!busy} onChange={(event) => void selectVersion(event.target.value)}>
            {versions.map((version, index) => (
              <option key={version.id} value={version.id}>
                v{version.number}{index === 0 ? " · Latest" : ""} — {version.filename}
              </option>
            ))}
          </select>
        </label>
        <label className="version-upload" htmlFor={inputId}>
          <Upload size={16} aria-hidden="true" /> Upload new version
          <input id={inputId} type="file" aria-label="Upload new version"
            accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            disabled={!!busy} onChange={upload} />
        </label>
        <div className="delete-split" role="group" aria-label="Delete résumé">
          <button type="button" disabled={!!busy} onClick={(event) => { openerRef.current = event.currentTarget; setError(null); setConfirm("version"); }}><Trash2 size={15} aria-hidden="true" /> Delete this version</button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger ref={menuTriggerRef} className="delete-menu-trigger" disabled={!!busy} aria-label="More delete options"><ChevronDown size={15} aria-hidden="true" /></DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content className="delete-menu" align="end" sideOffset={6}
                onCloseAutoFocus={(event) => { if (openingConfirmation.current) { event.preventDefault(); openingConfirmation.current = false; } }}>
                <DropdownMenu.Item className="delete-menu-item" disabled={!!busy} onSelect={() => {
                  openerRef.current = menuTriggerRef.current;
                  openingConfirmation.current = true;
                  setError(null);
                  setConfirm("all");
                }}><Trash2 size={15} aria-hidden="true" /> Delete all versions</DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
      </div>
      <p className="version-note">{versions.length} {versions.length === 1 ? "version" : "versions"} · PDF or DOCX · Max 5 MB</p>
      {busy ? <p className="version-feedback" role="status"><LoaderCircle className="busy-icon" size={16} aria-hidden="true" />{busy === "upload" ? "Uploading…" : busy === "delete" ? "Deleting…" : "Loading version…"}</p> : null}
      {notice ? <p className="version-feedback success" role="status"><CheckCircle2 size={16} aria-hidden="true" />{notice}</p> : null}
      <AlertDialog.Root open={confirm !== null} onOpenChange={(open) => { if (!open && !busy) { setConfirm(null); setError(null); } }}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="confirmation-overlay" />
          <AlertDialog.Content className="confirmation-dialog"
            onOpenAutoFocus={(event) => { event.preventDefault(); cancelRef.current?.focus(); }}
            onCloseAutoFocus={(event) => { event.preventDefault(); openerRef.current?.focus(); }}
            onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }}>
            <span className="confirmation-icon"><AlertTriangle aria-hidden="true" /></span>
            <AlertDialog.Title className="confirmation-title">{confirm === "all" ? "Delete all versions?" : `Delete version ${selected.number}?`}</AlertDialog.Title>
            <AlertDialog.Description className="confirmation-description">{confirm === "all"
             ? `This will permanently delete this résumé, all ${versions.length} ${versions.length === 1 ? "version" : "versions"}, and their results. This cannot be undone.`
            : `This will permanently delete ${selected.filename} (v${selected.number}) and its results. ${versions.length === 1 ? "This is the last version, so the résumé will also be removed. " : "Other versions will not be affected. "}This cannot be undone.`}</AlertDialog.Description>
            {error ? <p className="error" role="alert">{error}</p> : null}
            <div className="confirmation-actions">
              <AlertDialog.Cancel ref={cancelRef} disabled={!!busy}>Cancel</AlertDialog.Cancel>
              <button className="confirm-delete" type="button" disabled={!!busy} onClick={() => void remove()}><Trash2 size={16} aria-hidden="true" />{busy === "delete" ? "Deleting…" : "Confirm deletion"}</button>
            </div>
            <AlertDialog.Cancel className="confirmation-close" aria-label="Close confirmation" disabled={!!busy}><X size={18} aria-hidden="true" /></AlertDialog.Cancel>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
      {error && !confirm ? <p className="error version-feedback" role="alert"><AlertTriangle size={16} aria-hidden="true" />{error}</p> : null}
    </section>
  );
}
