import { isRedirect, useNavigate, useRouter } from "@tanstack/react-router";
import { AlertTriangle, LoaderCircle, Trash2 } from "lucide-react";
import { AlertDialog } from "radix-ui";
import { useState } from "react";

import { removeResume } from "~/fns/resumes.functions";

export function DeleteResume({ id, filename }: { id: string; filename: string }) {
  const router = useRouter();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function remove() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await removeResume({ data: { id } });
      if (!result.ok) { setError(result.message); return; }
      await router.invalidate();
      setOpen(false);
    } catch (cause) {
      if (isRedirect(cause)) await navigate({ to: "/login" });
      else setError("Could not delete this résumé. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <AlertDialog.Root open={open} onOpenChange={(next) => { if (!busy) { setOpen(next); setError(null); } }}>
      <AlertDialog.Trigger className="row-delete" aria-label={`Delete ${filename}`} title="Delete all versions"><Trash2 size={16} aria-hidden="true" /></AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="confirmation-overlay" />
        <AlertDialog.Content className="confirmation-dialog" onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }}>
          <span className="confirmation-icon"><AlertTriangle aria-hidden="true" /></span>
          <AlertDialog.Title className="confirmation-title">Delete this résumé?</AlertDialog.Title>
          <AlertDialog.Description className="confirmation-description">This will permanently delete {filename}, all its versions, and their results. This cannot be undone.</AlertDialog.Description>
          {error ? <p className="error" role="alert">{error}</p> : null}
          <div className="confirmation-actions">
            <AlertDialog.Cancel disabled={busy}>Cancel</AlertDialog.Cancel>
            <button type="button" className="confirm-delete" disabled={busy} onClick={() => void remove()}>
              {busy ? <LoaderCircle className="busy-icon" size={16} aria-hidden="true" /> : <Trash2 size={16} aria-hidden="true" />}
              {busy ? "Deleting…" : "Delete all versions"}
            </button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
