import { Outlet, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/login")({
  component: LoginLayout,
});

function LoginLayout() {
  return <Outlet />;
}
