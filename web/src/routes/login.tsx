import { createFileRoute, redirect } from "@tanstack/react-router";
import { useState, type FormEvent } from "react";

import { Page } from "~/components/page";
import { getAccount, sendMagicLink } from "~/fns/auth.functions";

export const Route = createFileRoute("/login")({
  beforeLoad: async () => {
    const account = await getAccount();
    if (account) throw redirect({ to: "/" });
  },
  component: LoginPage,
});

function LoginPage() {
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await sendMagicLink({ data: email });
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
      <header className="top">
        <div className="wordmark">Resume analyzer</div>
      </header>
      {sent ? (
        <div className="sign">
          <h1>Check your email</h1>
          <p>A sign-in link is on its way. It expires in fifteen minutes.</p>
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
          <h1>Sign in</h1>
          <p>We’ll email you a link. No password.</p>
          <label>
            Email
            <input
              type="email"
              name="email"
              autoComplete="email"
              required
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
          </button>
        </form>
      )}
    </Page>
  );
}
