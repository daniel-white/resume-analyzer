import { defineConfig, devices } from "@playwright/test";

const port = Number(process.env.PLAYWRIGHT_PORT ?? 4173);
const apiPort = Number(process.env.PLAYWRIGHT_API_PORT ?? 4181);
const api = `http://127.0.0.1:${apiPort}`;
const baseURL = `http://127.0.0.1:${port}`;
const production = process.env.PLAYWRIGHT_PRODUCTION === "1";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL,
    trace: "on-first-retry",
  },
  webServer: [
    {
      command: "node e2e/stub.mjs",
      url: `${api}/healthz`,
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
      env: {
        PLAYWRIGHT_API_PORT: String(apiPort),
      },
    },
    {
      command: production
        ? "node .output/server/index.mjs"
        : `pnpm exec vite dev --host 127.0.0.1 --port ${port}`,
      url: baseURL,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
      env: {
        API_URL: api,
        PORT: String(port),
        HOST: "127.0.0.1",
      },
    },
  ],
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    // WebKit (Safari) runs resume-viewer.spec.ts and theme.spec.ts.
    // It requires GTK/GStreamer/Cairo/Pango system libraries that are not
    // available in the default CI container. Enable with PLAYWRIGHT_WEBKIT=1.
    ...(process.env.PLAYWRIGHT_WEBKIT === "1"
      ? [
          {
            name: "webkit",
            testMatch: ["resume-viewer.spec.ts", "theme.spec.ts"],
            use: { ...devices["Desktop Safari"] },
          },
        ]
      : []),
  ],
});
