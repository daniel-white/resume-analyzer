import { createServerFn } from "@tanstack/react-start";

import { fetchBackendBuild, type BuildInfo } from "../server/version";

export type { BuildInfo };

export const getBackendBuild = createServerFn({ method: "GET" }).handler(
  async (): Promise<BuildInfo | null> => {
    const base = process.env["API_URL"] ?? "http://127.0.0.1:8080";
    return fetchBackendBuild(base);
  },
);
