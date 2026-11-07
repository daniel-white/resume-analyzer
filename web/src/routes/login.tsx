import { Outlet, createFileRoute } from "@tanstack/react-router";
import { resumeReturnTo } from "~/lib/return-to";

export const Route = createFileRoute("/login")({
  validateSearch: (search: Record<string, unknown>): { returnTo?: string } => ({ returnTo: resumeReturnTo(search.returnTo) }),
  component: LoginLayout,
});

function LoginLayout() {
  return <Outlet />;
}
