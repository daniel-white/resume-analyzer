import { createFileRoute } from "@tanstack/react-router";

// Same idea as the owner's /api/resumes/$id/events proxy, but admins review
// resumes they do not own. The Rust endpoint performs the admin check.
export const Route = createFileRoute("/api/admin/events")({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const base = process.env.API_URL ?? "http://127.0.0.1:8080";
        const headers = new Headers({ accept: "text/event-stream" });
        for (const name of ["cookie", "last-event-id"]) {
          const value = request.headers.get(name);
          if (value) headers.set(name, value);
        }
        try {
          const upstream = await fetch(new URL("/api/admin/events", base), {
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
          return new Response("Live diagnoses are unavailable.", {
            status: 502,
            headers: { "cache-control": "private, no-store" },
          });
        }
      },
    },
  },
});
