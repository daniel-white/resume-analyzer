import { Outlet, createFileRoute } from "@tanstack/react-router";
import { safeReturnTo } from "~/lib/return-to";

export const Route = createFileRoute("/login")({
  validateSearch: (search: Record<string, unknown>): { returnTo?: string } => ({ returnTo: safeReturnTo(search.returnTo) }),
  component: LoginLayout,
});

function LoginLayout() {
  return <Outlet />;
}
