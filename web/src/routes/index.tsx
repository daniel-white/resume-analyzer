import { createFileRoute } from "@tanstack/react-router";

import { getHealth } from "~/fns/health.functions";
import { summarize } from "~/server/health";

export const Route = createFileRoute("/")({
  loader: () => getHealth(),
  component: Home,
});

function Home() {
  const health = Route.useLoaderData();
  return (
    <main className="mx-auto max-w-xl px-6 py-16">
      <p className="text-sm text-muted-foreground">Dependency check</p>
      <h1 className="mt-2 text-3xl">Resume analyzer</h1>
      <p className="mt-6 font-mono text-sm">{summarize(health)}</p>
    </main>
  );
}
