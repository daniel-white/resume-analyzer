import { createFileRoute, Link } from "@tanstack/react-router";
import { BookOpen, X } from "lucide-react";

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
      <section className="about-window" aria-labelledby="about-title">
        <header className="about-titlebar">
          <span className="about-titlebar-icon" aria-hidden="true"><BookOpen size={14} /></span>
          <h1 id="about-title">About The Other Read</h1>
          <Link to="/" className="about-close" aria-label="Close and return to your résumés"><X size={15} aria-hidden="true" /></Link>
        </header>

        <div className="about-content">
          <div className="about-hero">
            <div className="about-app-icon" aria-hidden="true"><BookOpen size={30} /></div>
            <div>
              <p className="about-app-name">The Other Read</p>
              <p className="about-tagline">A clearer read on every résumé.</p>
              <div className="about-badges">
                <span className="about-badge">BETA</span>
                {dev ? <span className="about-badge about-badge-dev">LOCAL DEV BUILD</span> : null}
              </div>
            </div>
          </div>

          <div className="about-pane">
            <h2 className="about-section-title">Build information</h2>
            <BuildRows frontend={frontendBuild} backend={backend} dev={dev} />

            <div className="about-license">
              <h2 className="about-section-title">License</h2>
              <p>MIT License. Copyright © 2026 The Other Read contributors.</p>
              <p>Permission is granted, free of charge, to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of this software, provided that the copyright notice and this permission notice are included in all copies or substantial portions of the software.</p>
              <p>The software is provided “as is”, without warranty of any kind.</p>
            </div>
          </div>

          <footer className="about-actions">
            <Link to="/" className="about-ok">OK</Link>
          </footer>
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
      <div><dt>Frontend version</dt><dd>{frontend.version}</dd></div>
      <div><dt>Backend API version</dt><dd>{backend?.version ?? "unavailable"}</dd></div>
      <div><dt>Frontend commit</dt><dd><code>{frontend.commit}</code></dd></div>
      <div><dt>Backend commit</dt><dd><code>{backend?.commit ?? "unavailable"}</code></dd></div>
      <div><dt>Build</dt><dd>{dev ? "local development" : "release"}</dd></div>
    </dl>
  );
}
