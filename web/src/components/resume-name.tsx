import { isRedirect, useNavigate, useRouter } from "@tanstack/react-router";
import { Pencil } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { updateResumeName } from "~/fns/resumes.functions";
import { Input } from "./ui/input";

export function ResumeName({ id, title }: { id: string; title: string }) {
  const router = useRouter();
  const navigate = useNavigate();
  const trigger = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(title);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!editing && restoreFocus.current) {
      trigger.current?.focus();
      restoreFocus.current = false;
    }
  }, [editing]);

  function close() {
    restoreFocus.current = true;
    setEditing(false);
    setError(null);
  }

  async function save() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await updateResumeName({ data: { id, title: name } });
      if (!result.ok) { setError(result.message); return; }
      await router.invalidate();
      close();
    } catch (cause) {
      if (isRedirect(cause)) await navigate({ to: "/login" });
      else setError("Could not rename your resume. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  if (editing) {
    return (
      <form className="resume-name-editor" onSubmit={(event) => { event.preventDefault(); void save(); }}
        onKeyDown={(event) => { if (event.key === "Escape" && !busy) { event.preventDefault(); close(); } }}>
        <label htmlFor="resume-set-name">Give your resume a name</label>
        <Input id="resume-set-name" autoFocus value={name} disabled={busy} required placeholder="e.g. My next chapter"
          aria-invalid={!!error} aria-describedby={error ? "resume-name-error" : undefined}
          onChange={(event) => setName(event.target.value)} />
        <div className="resume-name-actions">
          <button type="submit" disabled={busy}>{busy ? "Saving…" : "Save name"}</button>
          <button type="button" disabled={busy} onClick={close}>Cancel</button>
        </div>
        {error ? <p id="resume-name-error" className="error" role="alert">{error}</p> : null}
      </form>
    );
  }

  return (
    <div className="resume-set-title">
      <h1 className="filename">{title}</h1>
      <button ref={trigger} type="button" className="rename-set" aria-label="Rename resume"
        onClick={() => { setName(title); setError(null); setEditing(true); }}>
        <Pencil size={14} aria-hidden="true" /> Rename
      </button>
    </div>
  );
}
