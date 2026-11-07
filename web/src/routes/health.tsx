import { createFileRoute } from "@tanstack/react-router";

import { getHealth } from "~/fns/health.functions";
import { summarize } from "~/server/health";
import { Page } from "~/components/page";
import { PublicHeader } from "~/components/public-header";

export const Route = createFileRoute("/health")({
  loader: () => getHealth(),
  component: HealthPage,
});

function HealthPage() {
  const health = Route.useLoaderData();
  return (
    <Page>
      <PublicHeader />
      <div className="library-heading">
        <p className="eyebrow">BEHIND THE OTHER READ</p>
        <h1>Dependency check</h1>
        <p>A quick look at the services that keep your next read running.</p>
      </div>
      <p className="health-summary sheet-face">{summarize(health)}</p>
    </Page>
  );
}
