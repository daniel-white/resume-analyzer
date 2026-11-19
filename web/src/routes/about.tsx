import { createFileRoute } from "@tanstack/react-router";
import { BookOpen } from "lucide-react";

import { Page } from "~/components/page";
import { PublicHeader } from "~/components/public-header";
import { getBackendBuild, type BuildInfo } from "~/fns/version.functions";
import { frontendBuild } from "~/server/version";

export const Route = createFileRoute("/about")({
  head: () => ({
    meta: [
      { title: "About — The Other Read" },
      {
        name: "description",
        content: "Version, build, license, and copyright information for The Other Read.",
      },
    ],
  }),
  loader: () => getBackendBuild(),
  component: AboutPage,
});

function AboutPage() {
  const backend = Route.useLoaderData();
  const dev = frontendBuild.dev || backend?.dev === true;

  return (
    <Page>
      <PublicHeader />
      <section className="about-page" aria-labelledby="about-title">
        <header className="about-intro">
          <div className="about-app-icon" aria-hidden="true"><BookOpen size={30} /></div>
          <div>
            <h1 id="about-title">About The Other Read</h1>
            <p className="about-tagline">A clearer read on every résumé.</p>
            <div className="about-badges">
              <span className="about-badge">BETA</span>
              {dev ? <span className="about-badge about-badge-dev">LOCAL DEV BUILD</span> : null}
            </div>
          </div>
        </header>

        <div className="about-sections">
          <section className="about-section" aria-labelledby="about-build-title">
            <h2 className="about-section-title" id="about-build-title">Build information</h2>
            <BuildRows frontend={frontendBuild} backend={backend} dev={dev} />
          </section>

          <section className="about-section" aria-labelledby="about-license-title">
            <h2 className="about-section-title" id="about-license-title">License</h2>
            <p>MIT License. Copyright © 2026 The Other Read contributors.</p>
            <p>Permission is granted, free of charge, to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of this software, provided that the copyright notice and this permission notice are included in all copies or substantial portions of the software.</p>
            <p>The software is provided “as is”, without warranty of any kind.</p>
          </section>
        </div>
      </section>
    </Page>
  );
}

function BuildRows({
  frontend,
  backend,
  dev,
}: {
  frontend: BuildInfo;
  backend: BuildInfo | null;
  dev: boolean;
}) {
  return (
    <dl className="about-rows">
      <BuildRow label="Frontend" info={frontend} dev={dev} />
      <BuildRow label="Backend" info={backend} dev={dev} />
    </dl>
  );
}

function BuildRow({ label, info, dev }: { label: string; info: BuildInfo | null; dev: boolean }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>
        <span className="about-version">{info?.version ?? "unavailable"}</span>
        <span className="about-separator" aria-hidden="true">·</span>
        <code title="Commit">{info?.commit ?? "unavailable"}</code>
        <span className="about-separator" aria-hidden="true">·</span>
        <span className="about-build-state">{dev ? "local development" : "release"}</span>
      </dd>
    </div>
  );
}
