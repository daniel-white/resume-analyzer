import { createFileRoute } from "@tanstack/react-router";
import { BookOpen } from "lucide-react";
import { Tabs } from "radix-ui";
import Markdown from "react-markdown";

import { Page } from "~/components/page";
import { PublicHeader } from "~/components/public-header";
import releaseNotes from "~/generated/release-notes.md?raw";
import { getBackendBuild, type BuildInfo } from "~/fns/version.functions";
import { frontendBuild } from "~/server/version";

export const Route = createFileRoute("/about")({
  head: () => ({
    meta: [
      { title: "About — The Other Read" },
      {
        name: "description",
        content: "Version, build, license, and release notes for The Other Read.",
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

        <Tabs.Root defaultValue="general" className="about-tabs">
          <Tabs.List className="about-tab-list" aria-label="About sections">
            <Tabs.Trigger className="about-tab" value="general">General</Tabs.Trigger>
            <Tabs.Trigger className="about-tab" value="changelog">Changelog</Tabs.Trigger>
            <Tabs.Trigger className="about-tab" value="license">License</Tabs.Trigger>
            <Tabs.Trigger className="about-tab" value="build">Build</Tabs.Trigger>
          </Tabs.List>

          <Tabs.Content className="about-tab-panel" value="general">
            <p>The Other Read turns a résumé into a clearer, structured read with a model you control.</p>
            <p>Copyright © 2026 The Other Read contributors. Released under the MIT License.</p>
          </Tabs.Content>

          <Tabs.Content className="about-tab-panel about-changelog" value="changelog">
            <Markdown allowedElements={["h2", "h3", "p", "ul", "li", "strong", "em"]} unwrapDisallowed>
              {releaseNotes}
            </Markdown>
          </Tabs.Content>

          <Tabs.Content className="about-tab-panel" value="license">
            <p>MIT License. Copyright © 2026 The Other Read contributors.</p>
            <p>Permission is granted, free of charge, to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of this software, provided that the copyright notice and this permission notice are included in all copies or substantial portions of the software.</p>
            <p>The software is provided “as is”, without warranty of any kind.</p>
          </Tabs.Content>

          <Tabs.Content className="about-tab-panel" value="build">
            <BuildRows frontend={frontendBuild} backend={backend} dev={dev} />
          </Tabs.Content>
        </Tabs.Root>
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
