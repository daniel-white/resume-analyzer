import { createFileRoute } from "@tanstack/react-router";

// The gateway routes /api straight to Rust. This streaming route also makes
// the standalone production web server work with a runtime backend address.
export const Route = createFileRoute("/api/resumes/$id/events")({
  server: {
    handlers: {
      GET: async ({ request, params }) => {
        const base = process.env.API_URL ?? "http://127.0.0.1:8080";
        const path = `/api/resumes/${encodeURIComponent(params.id)}/events`;
        const headers = new Headers({ accept: "text/event-stream" });
        for (const name of ["cookie", "last-event-id"]) {
          const value = request.headers.get(name);
          if (value) headers.set(name, value);
        }
        try {
          const upstream = await fetch(new URL(path, base), {
            headers,
            signal: request.signal,
            redirect: "error",
            cache: "no-store",
          });
          const result = new Headers({
            "cache-control": "private, no-store",
            "x-accel-buffering": "no",
          });
          const contentType = upstream.headers.get("content-type");
          if (contentType) result.set("content-type", contentType);
          return new Response(upstream.body, { status: upstream.status, headers: result });
        } catch {
          return new Response("Live analysis is unavailable.", {
            status: 502,
            headers: { "cache-control": "private, no-store" },
          });
        }
      },
    },
  },
});
