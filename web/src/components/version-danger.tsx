import { useRef, useState } from "react";
import { AlertDialog } from "radix-ui";
import { AlertTriangle, LoaderCircle, Trash2, X } from "lucide-react";

import { useResumeDetail } from "./resume-detail-context";

/** The only delete surface: one version, scoped to the version being viewed. */
export function VersionDanger() {
  const {
    view,
    versionNumber,
    busy,
    error,
    deleteVersion,
    clearError,
  } = useResumeDetail();
  const [confirm, setConfirm] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const openerRef = useRef<HTMLButtonElement>(null);
  const selected = view.versions.find((version) => version.id === view.versionId);
  const last = view.versions.length === 1;

  function open() {
    clearError();
    setConfirm(true);
  }

  return (
    <section className="resume-detail-danger" aria-label="Danger area">
      <div>
        <h2>Delete this version</h2>
        <p>
          Removes this version and its results. {last
            ? "It is the last version, so the résumé will also be removed. "
            : "Other versions will not be affected. "}
          This cannot be undone.
        </p>
      </div>
      <button type="button" className="danger-button" ref={openerRef} disabled={busy !== null} onClick={open}>
        <Trash2 size={15} aria-hidden="true" /> Delete this version
      </button>
      <AlertDialog.Root open={confirm} onOpenChange={(open) => {
        if (!open && busy === null) {
          setConfirm(false);
          clearError();
        }
      }}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="confirmation-overlay" />
          <AlertDialog.Content
            className="confirmation-dialog"
            onOpenAutoFocus={(event) => { event.preventDefault(); cancelRef.current?.focus(); }}
            onCloseAutoFocus={(event) => { event.preventDefault(); openerRef.current?.focus(); }}
            onEscapeKeyDown={(event) => { if (busy !== null) event.preventDefault(); }}
          >
            <span className="confirmation-icon"><AlertTriangle aria-hidden="true" /></span>
            <AlertDialog.Title className="confirmation-title">Delete version {versionNumber}?</AlertDialog.Title>
            <AlertDialog.Description className="confirmation-description">
              This will permanently delete {selected?.filename ?? "this version"} (v{versionNumber}) and its results.{" "}
              {last ? "This is the last version, so the résumé will also be removed. " : "Other versions will not be affected. "}
              This cannot be undone.
            </AlertDialog.Description>
            {error ? <p className="error" role="alert">{error}</p> : null}
            <div className="confirmation-actions">
              <AlertDialog.Cancel ref={cancelRef} disabled={busy !== null}>Cancel</AlertDialog.Cancel>
              <button className="confirm-delete" type="button" disabled={busy !== null} onClick={() => void deleteVersion()}>
                {busy === "delete" ? <LoaderCircle className="busy-icon" size={16} aria-hidden="true" /> : <Trash2 size={16} aria-hidden="true" />}
                {busy === "delete" ? "Deleting…" : "Confirm deletion"}
              </button>
            </div>
            <AlertDialog.Cancel className="confirmation-close" aria-label="Close confirmation" disabled={busy !== null}>
              <X size={18} aria-hidden="true" />
            </AlertDialog.Cancel>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </section>
  );
}
