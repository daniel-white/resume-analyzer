import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { nitro } from "nitro/vite";
import { defineConfig } from "vite";

const api = process.env.API_URL ?? "http://127.0.0.1:8080";

export default defineConfig({
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
