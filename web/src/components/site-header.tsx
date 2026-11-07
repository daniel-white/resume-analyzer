import { Link, useNavigate, useRouter } from "@tanstack/react-router";
import { useState } from "react";

import { signOut } from "~/fns/auth.functions";

export function SiteHeader({ email }: { email: string }) {
  const navigate = useNavigate();
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  async function onSignOut() {
    if (busy) return;
    setBusy(true);
    try {
      await signOut();
    } catch {
      setBusy(false);
      return;
    }
    await navigate({ to: "/login", replace: true });
    await router.invalidate();
  }

  return (
    <header className="top">
      <div className="flex min-w-0 items-baseline gap-4">
        <Link to="/" className="wordmark">The Other Read</Link>
        <Link to="/" className="text-sm text-[#d5deee]">Your resumes</Link>
      </div>
      <div className="who">
        <span>{email}</span>
        <button type="button" onClick={onSignOut} disabled={busy}>
          {busy ? "Signing out" : "Sign out"}
        </button>
      </div>
    </header>
  );
}
