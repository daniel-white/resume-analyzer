import { createFileRoute, redirect } from "@tanstack/react-router";
import { useEffect, useState, type FormEvent } from "react";

import { Page } from "~/components/page";
import { PublicHeader } from "~/components/public-header";
import { ArrowRight, Mail, ShieldCheck } from "lucide-react";
import { detectTimeZone } from "~/lib/timezone";
import { getAccount, sendMagicLink } from "~/fns/auth.functions";

export const Route = createFileRoute("/login/")({
  beforeLoad: async ({ search }) => {
    const account = await getAccount();
    if (account) throw redirect({ href: search.returnTo ?? "/" });
  },
  component: LoginPage,
});

function LoginPage() {
  const { returnTo } = Route.useSearch();
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);
  const [timezone, setTimezone] = useState<string | null>(null);

  useEffect(() => {
    setTimezone(detectTimeZone());
  }, []);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await sendMagicLink({ data: { email, returnTo, timezone: timezone ?? "" } });
      if (!result.ok) {
        setError(result.message);
        setBusy(false);
        return;
      }
      setSent(true);
    } catch {
      setError("The sign-in email did not send.");
      setBusy(false);
    }
  }

  return (
    <Page>
      <PublicHeader />
      <div className="auth-intro"><p className="eyebrow">ONE LINK. A DIFFERENT PERSPECTIVE.</p><h2>Your next chapter<br />starts <em>here.</em></h2><p>See the other side of your résumé. Keep your results together.<br />Make your next version count.</p></div>
      {sent ? (
        <div className="sign" role="status">
          <span className="sign-icon"><Mail size={26} aria-hidden="true" /></span>
          <h1>Check your email</h1>
          <p>A sign-in link is on its way to <strong>{email}</strong>. It expires in fifteen minutes.</p>
          <button
            type="button"
            className="text"
            onClick={() => {
              setSent(false);
              setBusy(false);
            }}
          >
            Use a different email
          </button>
        </div>
      ) : (
        <form className="sign" onSubmit={onSubmit}>
          <span className="sign-icon"><Mail size={26} aria-hidden="true" /></span>
          <h1>Sign in</h1>
          <p>We’ll email you a link. No password.</p>
          <label>
            Email
            <input
              type="email"
              name="email"
              autoComplete="email"
              required
              placeholder="you@example.com"
              disabled={busy}
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </label>
          {error ? (
            <p className="error" role="alert">
              {error}
            </p>
          ) : null}
          <button type="submit" disabled={busy}>
            {busy ? "Sending" : "Email me a link"}
            <ArrowRight size={18} aria-hidden="true" />
          </button>
          <p className="sign-note"><ShieldCheck size={15} aria-hidden="true" /> No password. Just a secure email link.</p>
        </form>
      )}
    </Page>
  );
}
