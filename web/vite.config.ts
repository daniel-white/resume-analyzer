import tailwindcss from "@tailwindcss/vite";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { nitro } from "nitro/vite";
import { defineConfig } from "vite";

const api = process.env["API_URL"] ?? "http://127.0.0.1:8080";
const webDir = dirname(fileURLToPath(import.meta.url));

function frontendVersion(): string {
  const manifest: unknown = JSON.parse(readFileSync(resolve(webDir, "package.json"), "utf8"));
  if (
    typeof manifest !== "object" ||
    manifest === null ||
    !("version" in manifest) ||
    typeof manifest.version !== "string"
  ) {
    throw new Error("web/package.json must contain a string version");
  }
  return manifest.version;
}

function shortCommit(): string {
  const supplied = process.env["FRONTEND_COMMIT"] ?? process.env["GIT_SHA"];
  if (supplied !== undefined) {
    return supplied.slice(0, 7);
  }
  try {
    return execFileSync("git", ["rev-parse", "--short=7", "HEAD"], {
      cwd: webDir,
      encoding: "utf8",
    }).trim();
  } catch {
    return "unknown";
  }
}

export default defineConfig({
  define: {
    __FRONTEND_VERSION__: JSON.stringify(frontendVersion()),
    __FRONTEND_COMMIT__: JSON.stringify(shortCommit()),
    __DEV_BUILD__: JSON.stringify(process.env["DEV_BUILD"] !== "false"),
  },
  server: {
    port: 3000,
    proxy: {
      // Browser calls stay on this origin. changeOrigin stays off so the
      // session cookie remains first-party.
      "/api": { target: api, changeOrigin: false },
    },
  },
  resolve: {
    tsconfigPaths: true,
  },
  plugins: [
    tailwindcss(),
    tanstackStart({ srcDirectory: "src" }),
    viteReact(),
    nitro(),
  ],
});
