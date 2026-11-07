import { createFileRoute } from "@tanstack/react-router";

import { getHealth } from "~/fns/health.functions";
import { summarize } from "~/server/health";

export const Route = createFileRoute("/health")({
  loader: () => getHealth(),
  component: HealthPage,
});

function HealthPage() {
  const health = Route.useLoaderData();
  return (
    <main className="wrap">
      <p className="who">Dependency check</p>
      <div className="filehead">
        <h1>Resume analyzer</h1>
      </div>
      <p className="check">{summarize(health)}</p>
    </main>
  );
}
