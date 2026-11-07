import { Link, useNavigate, useRouter } from "@tanstack/react-router";
import { useState } from "react";
import { DropdownMenu } from "radix-ui";
import { BookOpen, ChevronDown, Files, LoaderCircle, LogOut, UserRound } from "lucide-react";

import { signOut } from "~/fns/auth.functions";

export function SiteHeader({ email }: { email: string }) {
  const navigate = useNavigate();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);

  async function onSignOut() {
    if (busy) return;
    setBusy(true);
    setError(false);
    try {
      await signOut();
    } catch {
      setBusy(false);
      setError(true);
      return;
    }
    await navigate({ to: "/login", replace: true });
    await router.invalidate();
  }

  return (
    <header className="site-navigation">
      <Link to="/" className="brand"><span className="brand-mark"><BookOpen size={21} aria-hidden="true" /></span><span className="wordmark">The Other Read<span className="brand-caption">A second look at your resume</span></span></Link>
      <nav aria-label="Main navigation">
        <Link to="/" className="nav-library" activeOptions={{ exact: true }} activeProps={{ className: "nav-library is-active", "aria-current": "page" }}><Files size={16} aria-hidden="true" /> Your resumes</Link>
      </nav>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger className="account-trigger" aria-label="Account menu" disabled={busy}>
          {busy ? <LoaderCircle className="busy-icon" size={17} aria-hidden="true" /> : <UserRound size={17} aria-hidden="true" />}
          <span className="account-email">{email}</span><ChevronDown size={14} aria-hidden="true" />
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className="account-menu" align="end" sideOffset={8}>
            <DropdownMenu.Label className="account-menu-label">Signed in as<span>{email}</span></DropdownMenu.Label>
            <DropdownMenu.Separator className="account-menu-separator" />
            <DropdownMenu.Item className="account-menu-item" onSelect={() => void onSignOut()} disabled={busy}><LogOut size={16} aria-hidden="true" />Sign out</DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
      {error ? <p className="nav-error" role="alert">Could not sign out. Please try again.</p> : null}
    </header>
  );
}
