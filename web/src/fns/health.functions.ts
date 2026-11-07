import { createServerFn } from "@tanstack/react-start";

import { fetchHealth, type HealthReport } from "../server/health";

export type { HealthReport };

export const getHealth = createServerFn({ method: "GET" }).handler(
  async (): Promise<HealthReport> => {
    const base = process.env.API_URL ?? "http://127.0.0.1:8080";
    return fetchHealth(base);
  },
);
