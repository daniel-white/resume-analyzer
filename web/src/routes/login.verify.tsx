import { createFileRoute, Link } from "@tanstack/react-router";

import { Page } from "~/components/page";
import { PublicHeader } from "~/components/public-header";
import { verifyLink } from "~/fns/auth.functions";

type VerifySearch = {
  token?: string;
};

export const Route = createFileRoute("/login/verify")({
  validateSearch: (search: Record<string, unknown>): VerifySearch => ({
    token: typeof search.token === "string" ? search.token : undefined,
  }),
  loaderDeps: ({ search }) => ({ token: search.token ?? "", returnTo: search.returnTo }),
  loader: async ({ deps }) => {
    if (!deps.token) return { message: "That link did not sign you in." };
    const result = await verifyLink({ data: deps });
    return { message: result.message };
  },
  pendingComponent: SigningIn,
  component: VerifyPage,
});

function SigningIn() {
  return (
    <Page>
      <PublicHeader />
      <div className="sign">
        <h1>Signing you in</h1>
        <p>One moment.</p>
      </div>
    </Page>
  );
}

function VerifyPage() {
  const { message } = Route.useLoaderData();
  const { returnTo } = Route.useSearch();

  return (
    <Page>
      <PublicHeader />
      <div className="sign">
        <h1>That link did not work</h1>
        <p>{message}</p>
        <Link to="/login" search={{ returnTo }}>Ask for another link</Link>
      </div>
    </Page>
  );
}
