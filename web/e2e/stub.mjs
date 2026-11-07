import { createServer } from "node:http";
import { randomUUID } from "node:crypto";
import { docx, pdf } from "./documents.mjs";

const port = Number(process.env.PLAYWRIGHT_API_PORT ?? 4181);

const plugins = [
  {
    id: "greenline",
    name: "Greenline",
    subtitle: "Standard headings and plain text. Ignores the header.",
  },
  {
    id: "levr",
    name: "Levr",
    subtitle: "Skills match only as exact tokens.",
  },
  {
    id: "smart_resume",
    name: "SmartResume",
    subtitle: "Keeps table text and headers, loses text boxes. Simulated, modeled on publicly described parser behavior.",
  },
];

const scenarios = new Map();

const server = createServer(async (request, response) => {
  const url = new URL(request.url ?? "/", `http://127.0.0.1:${port}`);
  const { pathname } = url;
  const method = request.method ?? "GET";

  try {
    if (pathname === "/healthz" && method === "GET") {
      return send(response, 200, "ok");
    }

    if (pathname === "/__seed" && method === "POST") {
      const body = await readJson(request);
      const id = randomUUID();
      scenarios.set(id, {
        signedIn: body.signedIn ?? true,
        resumes: body.resumes ?? [],
        magicLinkStatus: body.magicLinkStatus ?? 202,
        verifyStatus: body.verifyStatus ?? 200,
        streams: new Set(),
        connections: 0,
        detailReads: 0,
      });
      return sendJson(response, 201, { id });
    }

    const control = pathname.match(/^\/__seed\/([^/]+)$/);
    if (control) {
      const seeded = scenarios.get(control[1]);
      if (!seeded) return send(response, 404, "");
      if (method === "GET") {
        return sendJson(response, 200, {
          connections: seeded.connections,
          streams: seeded.streams.size,
          detailReads: seeded.detailReads,
        });
      }
      if (method === "POST") {
        const body = await readJson(request);
        if (body.resumes) seeded.resumes = body.resumes;
        if (body.onConnectResumes) seeded.onConnectResumes = body.onConnectResumes;
        for (const stream of seeded.streams) {
          if (body.event) stream.write(`event: ${body.event}\ndata: {}\n\n`);
          if (body.disconnect) stream.end();
        }
        return send(response, 204, "");
      }
    }

    if (pathname === "/api/plugins" && method === "GET") {
      return sendJson(response, 200, { plugins });
    }

    const scenario = scenarios.get(seedId(request));
    if (!scenario) return sendJson(response, 401, { error: "unauthorized" });

    if (pathname === "/api/me" && method === "GET") {
      if (!scenario.signedIn) return sendJson(response, 401, { error: "unauthorized" });
      return sendJson(response, 200, {
        id: "user-ada",
        email: "ada@example.com",
        sessions: [],
      });
    }

    if (pathname === "/api/auth/magic-link" && method === "POST") {
      if (scenario.magicLinkStatus !== 202) {
        return sendJson(response, scenario.magicLinkStatus, { error: "email" });
      }
      return send(response, 202, "");
    }

    if (pathname === "/api/auth/magic-link/verify" && method === "POST") {
      if (scenario.verifyStatus !== 200) {
        return sendJson(response, scenario.verifyStatus, { error: "link" });
      }
      scenario.signedIn = true;
      return sendJson(
        response,
        200,
        { id: "user-ada", email: "ada@example.com" },
        { "set-cookie": "session=playwright-session; HttpOnly; SameSite=Lax; Path=/" },
      );
    }

    if (pathname === "/api/auth/logout" && method === "POST") {
      scenario.signedIn = false;
      return send(response, 204, "", {
        "set-cookie": "session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
      });
    }

    if (pathname === "/api/resumes" && method === "GET") {
      if (!scenario.signedIn) return sendJson(response, 401, { error: "unauthorized" });
      return sendJson(response, 200, { resumes: scenario.resumes.map(summary) });
    }

    if (pathname === "/api/resumes" && method === "POST") {
      if (!scenario.signedIn) return sendJson(response, 401, { error: "unauthorized" });
      const filename = (await filenameFrom(request)) ?? "dropped.pdf";
      const created = resume({
        id: "resume-uploaded",
        filename,
        status: "uploaded",
        parse: null,
        runs: [],
      });
      scenario.resumes = [created, ...scenario.resumes];
      return sendJson(response, 201, {
        id: created.id,
        version_id: created.versions[0].id,
        status: "uploaded",
      });
    }

    const file = pathname.match(/^\/api\/resumes\/([^/]+)\/versions\/([^/]+)\/file$/);
    if (file && method === "GET") {
      if (!scenario.signedIn) return sendJson(response, 401, { error: "unauthorized" });
      const found = scenario.resumes.find((item) => item.id === decodeURIComponent(file[1]));
      const version = found?.versions.find((item) => item.id === decodeURIComponent(file[2]));
      if (!version) return sendJson(response, 404, { error: "missing" });
      return send(response, 200, version.media_type === "application/pdf" ? pdf() : docx(), {
        "content-type": version.media_type,
        "cache-control": "private, no-store",
      });
    }

    const events = pathname.match(/^\/api\/resumes\/([^/]+)\/events$/);
    if (events && method === "GET") {
      if (!scenario.signedIn) return sendJson(response, 401, { error: "unauthorized" });
      if (!scenario.resumes.some((item) => item.id === decodeURIComponent(events[1]))) {
        return sendJson(response, 404, { error: "missing" });
      }
      response.writeHead(200, { "content-type": "text/event-stream", "cache-control": "private, no-store" });
      if (scenario.onConnectResumes) {
        scenario.resumes = scenario.onConnectResumes;
        delete scenario.onConnectResumes;
      }
      response.write(": connected\n\n");
      scenario.connections++;
      scenario.streams.add(response);
      response.on("close", () => scenario.streams.delete(response));
      return;
    }

    const detail = pathname.match(/^\/api\/resumes\/([^/]+)$/);
    if (detail && method === "GET") {
      if (!scenario.signedIn) return sendJson(response, 401, { error: "unauthorized" });
      const found = scenario.resumes.find((item) => item.id === decodeURIComponent(detail[1]));
      if (!found) return sendJson(response, 404, { error: "missing" });
      scenario.detailReads++;
      return sendJson(response, 200, found);
    }

    return sendJson(response, 404, { error: "unmocked", path: pathname, method });
  } catch (error) {
    console.error(error);
    return sendJson(response, 500, { error: "stub" });
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`playwright api stub on http://127.0.0.1:${port}`);
});

function seedId(request) {
  const cookie = request.headers.cookie ?? "";
  const match = cookie.match(/(?:^|;\s*)pw=([^;]+)/);
  return match?.[1] ?? "";
}

function summary(item) {
  const latest = item.versions[0];
  return {
    id: item.id,
    title: item.title,
    created_at: item.created_at,
    latest: {
      id: latest.id,
      number: latest.number,
      filename: latest.filename,
      status: latest.status,
      created_at: latest.created_at,
    },
  };
}

function resume(input) {
  return {
    id: input.id,
    title: input.filename,
    created_at: "2026-11-07T18:00:00Z",
    versions: [
      {
        id: `${input.id}-v1`,
        number: 1,
        filename: input.filename,
        media_type: "application/pdf",
        status: input.status,
        created_at: "2026-11-07T18:00:00Z",
        parse: input.parse,
        runs: input.runs,
      },
    ],
  };
}

function readJson(request) {
  return readBody(request).then((raw) => (raw ? JSON.parse(raw) : {}));
}

function readBody(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    request.on("error", reject);
  });
}

async function filenameFrom(request) {
  const raw = await readBody(request);
  const match = raw.match(/filename="([^"]+)"/);
  return match?.[1] ?? null;
}

function send(response, status, body, headers = {}) {
  response.writeHead(status, { "content-type": "text/plain; charset=utf-8", ...headers });
  response.end(body);
}

function sendJson(response, status, body, headers = {}) {
  response.writeHead(status, { "content-type": "application/json", ...headers });
  response.end(JSON.stringify(body));
}
