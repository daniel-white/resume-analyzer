import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/api/resumes/$id/versions/$versionId/file")({
  server: {
    handlers: {
      GET: async ({ request, params }) => {
        const base = process.env.API_URL ?? "http://127.0.0.1:8080";
        const path = `/api/resumes/${encodeURIComponent(params.id)}/versions/${encodeURIComponent(params.versionId)}/file`;
        const headers = new Headers();
        const cookie = request.headers.get("cookie");
        if (cookie) headers.set("cookie", cookie);
        try {
          const upstream = await fetch(new URL(path, base), {
            headers,
            signal: request.signal,
            redirect: "error",
            cache: "no-store",
          });
          const result = new Headers({ "cache-control": "private, no-store" });
          for (const name of ["content-type", "content-disposition", "x-content-type-options"]) {
            const value = upstream.headers.get(name);
            if (value) result.set(name, value);
          }
          return new Response(upstream.body, { status: upstream.status, headers: result });
        } catch {
          return new Response("This file could not be loaded.", {
            status: 502,
            headers: { "cache-control": "private, no-store" },
          });
        }
      },
    },
  },
});
